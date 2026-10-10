use std::path::Path;

use crate::app_paths;
use crate::collection::load_player_templates_for_datapack;
use crate::data::datapacks::{
    DatapackBundle, DatapackCatalog, MediaReferences, resolve_media_assets,
};
use crate::game::actions::ItemUseEffect;
use crate::game::derived::{
    ObjectiveConditionRowStyle, attractor_summary_rows, objective_condition_rows, run_phase_label,
    security_summary_rows, utility_relevance_rows,
};
use crate::game::{GameEvent, RunState};
use crate::media_pool::load_player_media_pool;

#[derive(Clone)]
pub struct DiagnosticReport {
    pub narrator_attached: bool,
    pub save_path: String,
    pub current_save_version: u32,
    pub loaded_save_version: Option<u32>,
    pub datapack_schema_version: String,
    pub loaded_run: bool,
    pub valid_datapacks: usize,
    pub invalid_datapacks: usize,
    pub active_bundle_name: Option<String>,
    pub template_counts: Option<TemplateCounts>,
    pub presentation_coverage: Option<PresentationCoverage>,
    pub media_asset_report: Option<MediaAssetReport>,
    pub player_library_report: PlayerLibraryReport,
    pub invalid_datapacks_detail: Vec<InvalidDatapackReport>,
    pub run_health: Option<RunHealthReport>,
    pub environment_checks: Vec<EnvironmentCheck>,
    pub warnings: Vec<DiagnosticMessage>,
    pub event_counters: EventCounters,
    pub recent_events: Vec<String>,
}

#[derive(Clone, Default)]
pub struct PlayerLibraryReport {
    pub template_rows: Vec<String>,
    pub media_rows: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone)]
pub struct TemplateCounts {
    pub locations: usize,
    pub items: usize,
    pub enemies: usize,
    pub bosses: usize,
    pub objectives: usize,
}

#[derive(Clone)]
pub struct PresentationCoverage {
    pub narrator_briefs: usize,
    pub media_references: usize,
}

#[derive(Clone)]
pub struct MediaAssetReport {
    pub present_assets: usize,
    pub missing_assets: usize,
    pub missing_entries: Vec<MediaAssetEntry>,
}

#[derive(Clone)]
pub struct MediaAssetEntry {
    pub template_kind: String,
    pub template_id: String,
    pub field_name: String,
    pub relative_path: String,
    pub resolved_path: String,
}

#[derive(Clone)]
pub struct InvalidDatapackReport {
    pub folder_name: String,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Clone)]
pub struct RunHealthReport {
    pub run_phase: String,
    pub location_id: String,
    pub hp: i32,
    pub max_hp: i32,
    pub noise_level: i32,
    pub objective_complete: bool,
    pub objective_condition_rows: Vec<String>,
    pub utility_relevance_rows: Vec<String>,
    pub security_summary_rows: Vec<String>,
    pub attractor_summary_rows: Vec<String>,
    pub inventory_items: usize,
    pub active_status_effects: usize,
    pub consumed_status_effects: usize,
    pub active_npcs: usize,
    pub additional_active_objectives: usize,
    pub active_temporary_exits: usize,
    pub player_media_enabled: bool,
    pub known_locations: usize,
    pub live_enemies: usize,
    pub live_bosses: usize,
    pub locked_locations: Vec<String>,
    pub broken_locked_locations: Vec<String>,
    pub barricaded_locations: Vec<String>,
}

#[derive(Clone)]
pub struct EnvironmentCheck {
    pub label: String,
    pub path: String,
    pub present: bool,
}

#[derive(Clone, Copy)]
pub enum DiagnosticSeverity {
    Info,
    Warn,
    Error,
}

impl DiagnosticSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Clone, Copy)]
pub enum DiagnosticArea {
    Content,
    Run,
    Runtime,
}

impl DiagnosticArea {
    pub fn label(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Run => "run",
            Self::Runtime => "runtime",
        }
    }
}

#[derive(Clone)]
pub struct DiagnosticMessage {
    pub severity: DiagnosticSeverity,
    pub area: DiagnosticArea,
    pub message: String,
}

impl DiagnosticMessage {
    pub fn info(area: DiagnosticArea, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Info,
            area,
            message: message.into(),
        }
    }

    pub fn warn(area: DiagnosticArea, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warn,
            area,
            message: message.into(),
        }
    }

    pub fn error(area: DiagnosticArea, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            area,
            message: message.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct EventCounters {
    pub moves: usize,
    pub attacks: usize,
    pub items_taken: usize,
    pub items_used: usize,
    pub waits: usize,
    pub wins: usize,
    pub losses: usize,
    pub rejections: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn build_diagnostic_report(
    datapacks: &DatapackCatalog,
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    narrator_attached: bool,
    save_path: &str,
    current_save_version: u32,
    loaded_save_version: Option<u32>,
    datapack_schema_version: &str,
    recent_events: &[GameEvent],
) -> DiagnosticReport {
    let invalid_datapacks_detail = datapacks
        .invalid
        .iter()
        .map(|invalid| InvalidDatapackReport {
            folder_name: invalid.folder_name.clone(),
            display_name: invalid.display_name.clone(),
            version: invalid.version.clone(),
            errors: invalid.errors.clone(),
        })
        .collect::<Vec<_>>();

    let template_counts = current_bundle.map(|bundle| TemplateCounts {
        locations: bundle.locations.len(),
        items: bundle.items.len(),
        enemies: bundle.enemies.len(),
        bosses: bundle.bosses.len(),
        objectives: bundle.objectives.len(),
    });

    let presentation_coverage = current_bundle.map(|bundle| PresentationCoverage {
        narrator_briefs: bundle
            .locations
            .iter()
            .filter(|entry| entry.narrator_brief.is_some())
            .count()
            + bundle
                .items
                .iter()
                .filter(|entry| entry.narrator_brief.is_some())
                .count()
            + bundle
                .enemies
                .iter()
                .filter(|entry| entry.narrator_brief.is_some())
                .count()
            + bundle
                .bosses
                .iter()
                .filter(|entry| entry.narrator_brief.is_some())
                .count(),
        media_references: bundle
            .locations
            .iter()
            .map(|entry| media_reference_count(&entry.media))
            .sum::<usize>()
            + bundle
                .items
                .iter()
                .map(|entry| media_reference_count(&entry.media))
                .sum::<usize>()
            + bundle
                .enemies
                .iter()
                .map(|entry| media_reference_count(&entry.media))
                .sum::<usize>()
            + bundle
                .bosses
                .iter()
                .map(|entry| media_reference_count(&entry.media))
                .sum::<usize>(),
    });

    let media_asset_report = current_bundle.map(build_media_asset_report);
    let player_library_report = build_player_library_report(current_bundle);

    let run_health = current_run.map(|run| {
        let (hp, max_hp) = run.controlled_actor_health();
        RunHealthReport {
            run_phase: run_phase_label(run).to_owned(),
            location_id: run.current_location_id.clone(),
            hp,
            max_hp,
            noise_level: run.noise_level,
            objective_complete: run.active_objective.completed,
            objective_condition_rows: objective_condition_rows(
                run,
                current_bundle,
                ObjectiveConditionRowStyle::Diagnostic,
            ),
            utility_relevance_rows: utility_relevance_rows(run, current_bundle),
            security_summary_rows: security_summary_rows(run, current_bundle),
            attractor_summary_rows: attractor_summary_rows(run, current_bundle),
            inventory_items: run.controlled_inventory().map_or(0, Vec::len),
            active_status_effects: run
                .status_effects
                .iter()
                .filter(|effect| matches!(effect.lifecycle, crate::game::InstanceLifecycle::Active))
                .count(),
            consumed_status_effects: run
                .status_effects
                .iter()
                .filter(|effect| {
                    matches!(effect.lifecycle, crate::game::InstanceLifecycle::Consumed)
                })
                .count(),
            active_npcs: run
                .location_npcs
                .values()
                .flatten()
                .filter(|npc| matches!(npc.lifecycle, crate::game::InstanceLifecycle::Active))
                .count(),
            additional_active_objectives: run
                .additional_objectives
                .iter()
                .filter(|objective| {
                    matches!(objective.lifecycle, crate::game::InstanceLifecycle::Active)
                })
                .count(),
            active_temporary_exits: run
                .temporary_exits
                .iter()
                .filter(|exit| matches!(exit.lifecycle, crate::game::InstanceLifecycle::Active))
                .count(),
            player_media_enabled: run.player_media_enabled,
            known_locations: run.known_locations.len(),
            live_enemies: run.enemies_alive.len(),
            live_bosses: run.bosses_alive.len(),
            locked_locations: {
                let mut locked = run.locked_locations.iter().cloned().collect::<Vec<_>>();
                locked.sort();
                locked
            },
            broken_locked_locations: {
                let mut broken = run
                    .broken_locked_locations
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();
                broken.sort();
                broken
            },
            barricaded_locations: {
                let mut barricaded = run.barricaded_locations.iter().cloned().collect::<Vec<_>>();
                barricaded.sort();
                barricaded
            },
        }
    });

    let environment_checks = [
        ("Assets datapacks", "assets/datapacks"),
        ("Runtime", "runtime"),
        ("Runtime saves", "runtime/saves"),
        ("Models", "models"),
        ("Datasets", "datasets"),
        ("Handoff", "handoff"),
    ]
    .into_iter()
    .map(|(label, path)| EnvironmentCheck {
        label: label.to_owned(),
        path: app_paths::display_path(path),
        present: app_paths::path(path).exists(),
    })
    .collect::<Vec<_>>();

    DiagnosticReport {
        narrator_attached,
        save_path: save_path.to_owned(),
        current_save_version,
        loaded_save_version,
        datapack_schema_version: datapack_schema_version.to_owned(),
        loaded_run: current_run.is_some(),
        valid_datapacks: datapacks.valid.len(),
        invalid_datapacks: datapacks.invalid.len(),
        active_bundle_name: current_bundle.map(|bundle| bundle.pack.display_name.clone()),
        template_counts,
        presentation_coverage,
        invalid_datapacks_detail,
        media_asset_report,
        player_library_report,
        run_health,
        environment_checks,
        warnings: collect_diagnostic_warnings(
            datapacks,
            current_bundle,
            current_run,
            save_path,
            current_save_version,
            loaded_save_version,
        ),
        event_counters: event_counters(recent_events),
        recent_events: recent_events.iter().rev().map(format_game_event).collect(),
    }
}

fn build_player_library_report(current_bundle: Option<&DatapackBundle>) -> PlayerLibraryReport {
    let mut report = PlayerLibraryReport::default();
    if let Some(bundle) = current_bundle {
        match load_player_templates_for_datapack(&bundle.pack.id) {
            Ok(items) => report.template_rows = items
                .into_iter()
                .map(|item| {
                    format!(
                        "Template {} ({}) | {} | tags: {} | controls: enable/disable, edit, remove",
                        item.name,
                        item.id,
                        if item.enabled { "enabled" } else { "disabled" },
                        item.tags.join(", ")
                    )
                })
                .collect(),
            Err(error) => report
                .errors
                .push(format!("Player template library: {error}")),
        }
    }
    match load_player_media_pool() {
        Ok(pool) => report.media_rows = pool
            .assets
            .into_iter()
            .map(|asset| {
                format!(
                    "Media {:?}: {} | {} | {} | tags: {} | controls: enable/disable, edit, remove",
                    asset.kind,
                    asset.path,
                    if asset.enabled { "enabled" } else { "disabled" },
                    if Path::new(&asset.path).is_file() {
                        "file present"
                    } else {
                        "MISSING FILE"
                    },
                    asset.tags.join(", ")
                )
            })
            .collect(),
        Err(error) => report.errors.push(format!("Player media library: {error}")),
    }
    report
}

fn media_reference_count(media: &crate::data::datapacks::MediaReferences) -> usize {
    [
        media.image.as_deref(),
        media.gif.as_deref(),
        media.video.as_deref(),
        media.audio.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .count()
}

fn build_media_asset_report(bundle: &DatapackBundle) -> MediaAssetReport {
    let mut present_assets = 0;
    let mut missing_entries = Vec::new();

    for location in &bundle.locations {
        audit_media_entry(
            bundle,
            "location",
            &location.id,
            &location.media,
            &mut present_assets,
            &mut missing_entries,
        );
    }
    for item in &bundle.items {
        audit_media_entry(
            bundle,
            "item",
            &item.id,
            &item.media,
            &mut present_assets,
            &mut missing_entries,
        );
    }
    for enemy in &bundle.enemies {
        audit_media_entry(
            bundle,
            "enemy",
            &enemy.id,
            &enemy.media,
            &mut present_assets,
            &mut missing_entries,
        );
    }
    for boss in &bundle.bosses {
        audit_media_entry(
            bundle,
            "boss",
            &boss.id,
            &boss.media,
            &mut present_assets,
            &mut missing_entries,
        );
    }

    MediaAssetReport {
        present_assets,
        missing_assets: missing_entries.len(),
        missing_entries,
    }
}

fn audit_media_entry(
    bundle: &DatapackBundle,
    template_kind: &str,
    template_id: &str,
    media: &MediaReferences,
    present_assets: &mut usize,
    missing_entries: &mut Vec<MediaAssetEntry>,
) {
    for asset in resolve_media_assets(bundle, media) {
        if asset.present {
            *present_assets += 1;
        } else {
            missing_entries.push(MediaAssetEntry {
                template_kind: template_kind.to_owned(),
                template_id: template_id.to_owned(),
                field_name: asset.field_name,
                relative_path: asset.relative_path,
                resolved_path: asset.resolved_path,
            });
        }
    }
}

fn collect_diagnostic_warnings(
    datapacks: &DatapackCatalog,
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    save_path: &str,
    current_save_version: u32,
    loaded_save_version: Option<u32>,
) -> Vec<DiagnosticMessage> {
    let mut warnings = Vec::new();

    if datapacks.valid.is_empty() {
        warnings.push(DiagnosticMessage::error(
            DiagnosticArea::Content,
            "No valid datapacks are currently available.",
        ));
    }
    if !datapacks.invalid.is_empty() {
        warnings.push(DiagnosticMessage::warn(
            DiagnosticArea::Content,
            format!("{} invalid datapack(s) detected.", datapacks.invalid.len()),
        ));
    }
    if current_bundle.is_none() {
        warnings.push(DiagnosticMessage::warn(
            DiagnosticArea::Content,
            "No active datapack bundle is loaded.",
        ));
    } else if let Some(bundle) = current_bundle {
        let media_report = build_media_asset_report(bundle);
        if media_report.missing_assets > 0 {
            warnings.push(DiagnosticMessage::warn(
                DiagnosticArea::Content,
                format!(
                    "{} referenced media asset(s) are missing from the active datapack.",
                    media_report.missing_assets
                ),
            ));
        }
    }
    if let Some(run) = current_run {
        if !run.controlled_actor_is_alive() {
            warnings.push(DiagnosticMessage::info(
                DiagnosticArea::Run,
                "The controlled actor is incapacitated at 0 HP; ordinary actions are blocked until revival.",
            ));
        }
        if run.active_objective.completed {
            warnings.push(DiagnosticMessage::info(
                DiagnosticArea::Run,
                "Current run is in a completed state.",
            ));
        }
        if run.known_locations.is_empty() {
            warnings.push(DiagnosticMessage::warn(
                DiagnosticArea::Run,
                "Run has no known locations recorded.",
            ));
        }
    } else {
        warnings.push(DiagnosticMessage::warn(
            DiagnosticArea::Run,
            "No active run is loaded.",
        ));
    }
    if !Path::new(save_path).exists() {
        warnings.push(DiagnosticMessage::info(
            DiagnosticArea::Runtime,
            "No save file exists at the configured runtime save path.",
        ));
    }
    if let Some(version) = loaded_save_version
        && version != current_save_version
    {
        warnings.push(DiagnosticMessage::warn(
            DiagnosticArea::Runtime,
            format!(
                "Loaded save version {} differs from current save version {}.",
                version, current_save_version
            ),
        ));
    }
    for required_path in [
        "assets/datapacks",
        "runtime",
        "models",
        "datasets",
        "handoff",
    ] {
        if !app_paths::path(required_path).exists() {
            warnings.push(DiagnosticMessage::error(
                DiagnosticArea::Runtime,
                format!("Expected folder '{}' is missing.", required_path),
            ));
        }
    }

    warnings
}

fn event_counters(events: &[GameEvent]) -> EventCounters {
    let mut counters = EventCounters::default();

    for event in events {
        match event {
            GameEvent::Moved { .. } => counters.moves += 1,
            GameEvent::AttackResolved { .. } | GameEvent::AttackWhiff { .. } => {
                counters.attacks += 1
            }
            GameEvent::ItemTaken { .. }
            | GameEvent::ItemTransferred {
                origin: crate::game::state::ItemOwner::Location(_),
                ..
            } => counters.items_taken += 1,
            GameEvent::ItemUsed { .. } | GameEvent::ItemConsumed { .. } => counters.items_used += 1,
            GameEvent::Waited { .. } => counters.waits += 1,
            GameEvent::RunWon => counters.wins += 1,
            GameEvent::RunLost { .. } => counters.losses += 1,
            GameEvent::NarratorItemAdmitted { .. } => {}
            GameEvent::NarratorStatusAdmitted { .. } => {}
            GameEvent::NarratorNpcAdmitted { .. } => {}
            GameEvent::NarratorEnemyAdmitted { .. } => {}
            GameEvent::NarratorBossAdmitted { .. } => {}
            GameEvent::NarratorTemporaryExitAdmitted { .. } => {}
            GameEvent::StatusEffectExpired { .. } => {}
            GameEvent::ActionRejected { .. } | GameEvent::SelectedPackFailedValidation { .. } => {
                counters.rejections += 1
            }
            _ => {}
        }
    }

    counters
}

fn format_game_event(event: &GameEvent) -> String {
    match event {
        GameEvent::HelpShown => "HelpShown".to_owned(),
        GameEvent::ActionRejected { reason } => format!("ActionRejected(reason={})", reason),
        GameEvent::SelectedPackFailedValidation {
            folder_name,
            reason,
        } => format!(
            "SelectedPackFailedValidation(folder_name={}, reason={})",
            folder_name, reason
        ),
        GameEvent::LocationLooked { location_id, .. } => {
            format!("LocationLooked(location_id={})", location_id)
        }
        GameEvent::Moved {
            from_location_id,
            to_location_id,
            ..
        } => format!("Moved(from={}, to={})", from_location_id, to_location_id),
        GameEvent::MovementBlocked {
            attempted_destination,
            ..
        } => format!(
            "MovementBlocked(attempted_destination={})",
            attempted_destination
        ),
        GameEvent::LocationUnlocked {
            location_id,
            item_id,
            ..
        } => format!(
            "LocationUnlocked(location_id={}, item_id={})",
            location_id, item_id
        ),
        GameEvent::LocationBarricaded {
            location_id,
            item_id,
            ..
        } => format!(
            "LocationBarricaded(location_id={}, item_id={})",
            location_id, item_id
        ),
        GameEvent::Inspected { target, .. } => format!("Inspected(target={})", target),
        GameEvent::ItemTaken { item_id, .. } => format!("ItemTaken(item_id={})", item_id),
        GameEvent::ItemTransferred {
            item_id,
            origin,
            destination,
        } => format!(
            "ItemTransferred(item_id={}, origin={}, destination={})",
            item_id, origin, destination
        ),
        GameEvent::SocialEventApplied {
            origin_id,
            subject_id,
            kind,
            relationship_delta,
            relationship_score,
            disposition,
        } => format!(
            "SocialEventApplied(origin={}, subject={}, kind={:?}, delta={}, score={}, disposition={:?})",
            origin_id, subject_id, kind, relationship_delta, relationship_score, disposition
        ),
        GameEvent::AbilityUsed {
            origin_id,
            grant_id,
            template_id,
            target_id,
            effect,
            remaining_uses,
        } => format!(
            "AbilityUsed(origin={}, grant={}, template={}, target={}, effect={}, remaining_uses={:?})",
            origin_id, grant_id, template_id, target_id, effect, remaining_uses
        ),
        GameEvent::AbilityCooldownSet {
            origin_id,
            grant_id,
            template_id,
            ready_at_turn,
        } => format!(
            "AbilityCooldownSet(origin={}, grant={}, template={}, ready_at_turn={})",
            origin_id, grant_id, template_id, ready_at_turn
        ),
        GameEvent::StatusEffectApplied {
            origin_id,
            instance_id,
            trigger,
            duration,
        } => format!(
            "StatusEffectApplied(origin={}, instance={}, trigger={:?}, duration={:?})",
            origin_id, instance_id, trigger, duration
        ),
        GameEvent::ConnectionsRevealed {
            origin_id,
            location_id,
            count,
        } => format!(
            "ConnectionsRevealed(origin={}, location={}, count={})",
            origin_id, location_id, count
        ),
        GameEvent::ProgressAwarded {
            origin_id,
            pool,
            amount,
            total,
        } => format!(
            "ProgressAwarded(origin={}, pool={}, amount={}, total={})",
            origin_id, pool, amount, total
        ),
        GameEvent::SkillLevelGained {
            origin_id,
            pool,
            level,
        } => format!(
            "SkillLevelGained(origin={}, pool={}, level={})",
            origin_id, pool, level
        ),
        GameEvent::MaxHpIncreased {
            origin_id,
            amount,
            max_hp,
        } => format!(
            "MaxHpIncreased(origin={}, amount={}, max_hp={})",
            origin_id, amount, max_hp
        ),
        GameEvent::AbilityGranted {
            owner_id,
            grant_id,
            template_id,
            source,
        } => format!(
            "AbilityGranted(owner={}, grant={}, template={}, source={:?})",
            owner_id, grant_id, template_id, source
        ),
        GameEvent::AbilityGrantExpired {
            owner_id,
            grant_id,
            template_id,
        } => format!(
            "AbilityGrantExpired(owner={}, grant={}, template={})",
            owner_id, grant_id, template_id
        ),
        GameEvent::ItemEquipped { origin_id, item_id } => {
            format!("ItemEquipped(origin={}, item_id={})", origin_id, item_id)
        }
        GameEvent::ItemUsed {
            origin_id,
            item_id,
            effect,
        } => match effect {
            ItemUseEffect::Healing { amount } => {
                format!(
                    "ItemUsed(origin={}, item_id={}, effect=Healing({}))",
                    origin_id, item_id, amount
                )
            }
            ItemUseEffect::RevealedLocations { count } => {
                format!(
                    "ItemUsed(origin={}, item_id={}, effect=RevealedLocations({}))",
                    origin_id, item_id, count
                )
            }
            ItemUseEffect::NoEffect => {
                format!(
                    "ItemUsed(origin={}, item_id={}, effect=NoEffect)",
                    origin_id, item_id
                )
            }
        },
        GameEvent::ItemConsumed {
            origin_id,
            item_id,
            template_id,
            reason,
        } => format!(
            "ItemConsumed(origin={}, item_id={}, template_id={}, reason={})",
            origin_id, item_id, template_id, reason
        ),
        GameEvent::AttackResolved {
            target_id,
            target_kind,
            damage,
            defeated,
            ..
        } => format!(
            "AttackResolved(target_id={}, kind={:?}, damage={}, defeated={})",
            target_id, target_kind, damage, defeated
        ),
        GameEvent::DamageTaken {
            origin_id,
            target_id,
            amount,
            remaining_hp,
        } => format!(
            "DamageTaken(origin={}, target={}, amount={}, remaining_hp={})",
            origin_id, target_id, amount, remaining_hp
        ),
        GameEvent::HealthRestored {
            origin_id,
            target_id,
            amount,
            remaining_hp,
        } => format!(
            "HealthRestored(origin={}, target={}, amount={}, remaining_hp={})",
            origin_id, target_id, amount, remaining_hp
        ),
        GameEvent::ActorSpawnedFromNoise {
            origin_id,
            template_id,
            location_id,
        } => format!(
            "ActorSpawnedFromNoise(origin_id={}, template_id={}, location_id={})",
            origin_id, template_id, location_id
        ),
        GameEvent::NoiseAttractorShifted {
            location_id,
            actor_ids,
        } => format!(
            "NoiseAttractorShifted(location_id={}, actor_ids={})",
            location_id,
            actor_ids.join("|")
        ),
        GameEvent::SightAttractorAcquired {
            origin_id,
            subject_id,
            location_id,
        } => format!(
            "SightAttractorAcquired(origin_id={}, subject_id={}, location_id={})",
            origin_id, subject_id, location_id
        ),
        GameEvent::SightAttractorMissed {
            origin_id,
            subject_id,
            location_id,
            detect_chance_percent,
            roll_percent,
        } => format!(
            "SightAttractorMissed(origin_id={}, subject_id={}, location_id={}, detect_chance_percent={}, roll_percent={})",
            origin_id, subject_id, location_id, detect_chance_percent, roll_percent
        ),
        GameEvent::SightAttractorLost {
            origin_id,
            subject_id,
        } => format!(
            "SightAttractorLost(origin_id={}, subject_id={})",
            origin_id, subject_id
        ),
        GameEvent::ActorMoved {
            origin_id,
            from_location_id,
            to_location_id,
            target_location_id,
        } => format!(
            "ActorMoved(origin_id={}, from={}, to={}, target={})",
            origin_id, from_location_id, to_location_id, target_location_id
        ),
        GameEvent::ActorWaited {
            origin_id,
            location_id,
            reason,
        } => format!(
            "ActorWaited(origin_id={}, location_id={}, reason={})",
            origin_id, location_id, reason
        ),
        GameEvent::ActorAttackedHazard {
            origin_id,
            hazard_kind,
            location_id,
            break_chance_percent,
            roll_percent,
            broken,
        } => format!(
            "ActorAttackedHazard(origin_id={}, hazard_kind={:?}, location_id={}, break_chance_percent={}, roll_percent={}, broken={})",
            origin_id, hazard_kind, location_id, break_chance_percent, roll_percent, broken
        ),
        GameEvent::AttackWhiff { .. } => "AttackWhiff".to_owned(),
        GameEvent::Waited { location_id, .. } => format!("Waited(location_id={})", location_id),
        GameEvent::ObjectiveCompleted { objective_id } => {
            format!("ObjectiveCompleted(objective_id={})", objective_id)
        }
        GameEvent::RunWon => "RunWon".to_owned(),
        GameEvent::RunLost { actor_id } => format!("RunLost(actor_id={})", actor_id),
        GameEvent::NarratorItemAdmitted { item_id } => {
            format!("NarratorItemAdmitted(item_id={})", item_id)
        }
        GameEvent::NarratorStatusAdmitted { instance_id } => {
            format!("NarratorStatusAdmitted(instance_id={})", instance_id)
        }
        GameEvent::NarratorNpcAdmitted { instance_id } => {
            format!("NarratorNpcAdmitted(instance_id={})", instance_id)
        }
        GameEvent::NarratorEnemyAdmitted { instance_id } => {
            format!("NarratorEnemyAdmitted(instance_id={})", instance_id)
        }
        GameEvent::NarratorBossAdmitted { instance_id } => {
            format!("NarratorBossAdmitted(instance_id={})", instance_id)
        }
        GameEvent::NarratorTemporaryExitAdmitted { instance_id } => {
            format!("NarratorTemporaryExitAdmitted(instance_id={})", instance_id)
        }
        GameEvent::StatusEffectExpired { instance_id } => {
            format!("StatusEffectExpired(instance_id={})", instance_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::{
        datapack_schema_version, discover_datapacks, load_datapack_bundle_by_folder,
    };
    use crate::game::actions::GameEvent;
    use crate::game::{
        ActorDisposition, ActorState, InstanceLifecycle, NpcInstance, generate_new_run,
    };

    use super::{DiagnosticArea, DiagnosticSeverity, build_diagnostic_report};

    #[test]
    fn diagnostic_report_summarizes_active_bundle_and_run() {
        let datapacks = discover_datapacks();
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let run = generate_new_run(&bundle).state;
        let events = vec![
            GameEvent::Moved {
                origin_id: "player".to_owned(),
                from_location_id: "front_verandah".to_owned(),
                to_location_id: "kitchen".to_owned(),
            },
            GameEvent::ItemTaken {
                origin_id: "player".to_owned(),
                item_id: "medkit".to_owned(),
            },
        ];

        let report = build_diagnostic_report(
            &datapacks,
            Some(&bundle),
            Some(&run),
            true,
            "runtime/saves/current_run.json",
            1,
            Some(1),
            datapack_schema_version(),
            &events,
        );

        assert!(report.narrator_attached);
        assert!(report.valid_datapacks >= 2);
        assert_eq!(
            report.active_bundle_name.as_deref(),
            Some("Property Siege Classic")
        );
        assert_eq!(
            report
                .run_health
                .as_ref()
                .map(|health| health.location_id.as_str()),
            Some("front_verandah")
        );
        assert!(report.run_health.as_ref().is_some_and(|health| {
            health
                .objective_condition_rows
                .iter()
                .any(|row| row == "Objective item 'House Keys (house_keys)': missing")
        }));
        assert!(report.run_health.as_ref().is_some_and(|health| {
            health
                .objective_condition_rows
                .iter()
                .any(|row| row == "Objective boss 'Garage Brute (brute_in_garage)': alive")
        }));
        assert!(report.run_health.as_ref().is_some_and(|health| {
            health
                .objective_condition_rows
                .iter()
                .any(|row| row == "Objective location 'Garage (garage)': not reached")
        }));
        assert!(report.run_health.as_ref().is_some_and(|health| {
            health.utility_relevance_rows.iter().any(|row| {
                row == "Utility: Torch (torch) reveals connected exits from the current room."
            })
        }));
        assert!(report.run_health.as_ref().is_some_and(|health| {
            health.security_summary_rows.iter().any(|row| {
                row == "Open approaches: Front Verandah (front_verandah), Back Garden (back_garden)"
            })
        }));
        assert!(
            report
                .run_health
                .as_ref()
                .is_some_and(|health| { health.attractor_summary_rows.is_empty() })
        );
        assert_eq!(report.event_counters.moves, 1);
        assert_eq!(report.event_counters.items_taken, 1);
        assert!(
            report
                .recent_events
                .iter()
                .any(|line| line.contains("ItemTaken(item_id=medkit)"))
        );
    }

    #[test]
    fn diagnostic_report_warns_when_media_assets_are_missing() {
        let datapacks = discover_datapacks();
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let run = generate_new_run(&bundle).state;

        let report = build_diagnostic_report(
            &datapacks,
            Some(&bundle),
            Some(&run),
            true,
            "runtime/saves/current_run.json",
            1,
            Some(1),
            datapack_schema_version(),
            &[],
        );

        assert!(
            report
                .media_asset_report
                .as_ref()
                .is_some_and(|media| media.missing_assets > 0)
        );
        assert!(report.warnings.iter().any(|warning| {
            matches!(warning.severity, DiagnosticSeverity::Warn)
                && matches!(warning.area, DiagnosticArea::Content)
                && warning
                    .message
                    .contains("referenced media asset(s) are missing")
        }));
    }

    #[test]
    fn diagnostic_report_surfaces_active_attractors() {
        let datapacks = discover_datapacks();
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut run = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        run.enemies_alive.insert(enemy_id.clone());
        run.enemy_hp.insert(enemy_id.clone(), 3);
        run.actor_sight_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        run.actor_sight_subjects
            .insert(enemy_id.clone(), "player".to_owned());

        let report = build_diagnostic_report(
            &datapacks,
            Some(&bundle),
            Some(&run),
            false,
            "save.json",
            1,
            Some(1),
            crate::data::datapacks::datapack_schema_version(),
            &[],
        );

        assert!(report.run_health.as_ref().is_some_and(|health| {
            health
                .attractor_summary_rows
                .iter()
                .any(|row| row.contains("tracking by sight toward Player"))
        }));
    }

    #[test]
    fn diagnostic_report_surfaces_a_tracking_npc_without_enemy_specific_logic() {
        let datapacks = discover_datapacks();
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut run = generate_new_run(&bundle).state;
        let mut actor = ActorState::configured(None, None, true, false, true, false);
        actor.add_declared_capabilities(&["noise_track".to_owned()]);
        run.location_npcs.insert(
            "kitchen".to_owned(),
            vec![NpcInstance {
                instance_id: "tracker_fixture".to_owned(),
                template_id: "tracker_fixture".to_owned(),
                location_id: "kitchen".to_owned(),
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Tracker".to_owned(),
                description: "A generic perception fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Neutral,
                actor,
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        run.actor_noise_targets
            .insert("tracker_fixture".to_owned(), "front_verandah".to_owned());

        let report = build_diagnostic_report(
            &datapacks,
            Some(&bundle),
            Some(&run),
            false,
            "save.json",
            1,
            Some(1),
            crate::data::datapacks::datapack_schema_version(),
            &[],
        );

        assert!(report.run_health.as_ref().is_some_and(|health| {
            health
                .attractor_summary_rows
                .iter()
                .any(|row| row.contains("Tracker (tracker_fixture) tracking noise"))
        }));
    }
}
