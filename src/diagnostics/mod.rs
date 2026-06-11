use std::path::Path;

use crate::data::datapacks::{
    DatapackBundle, DatapackCatalog, MediaReferences, resolve_media_assets,
};
use crate::game::actions::ItemUseEffect;
use crate::game::{GameEvent, RunState};

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
    pub invalid_datapacks_detail: Vec<InvalidDatapackReport>,
    pub run_health: Option<RunHealthReport>,
    pub environment_checks: Vec<EnvironmentCheck>,
    pub warnings: Vec<DiagnosticMessage>,
    pub event_counters: EventCounters,
    pub recent_events: Vec<String>,
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
    pub errors: Vec<String>,
}

#[derive(Clone)]
pub struct RunHealthReport {
    pub location_id: String,
    pub hp: i32,
    pub max_hp: i32,
    pub objective_complete: bool,
    pub inventory_items: usize,
    pub known_locations: usize,
    pub live_enemies: usize,
    pub live_bosses: usize,
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

    let run_health = current_run.map(|run| RunHealthReport {
        location_id: run.current_location_id.clone(),
        hp: run.hp,
        max_hp: run.max_hp,
        objective_complete: run.active_objective.completed,
        inventory_items: run.inventory.len(),
        known_locations: run.known_locations.len(),
        live_enemies: run.enemies_alive.len(),
        live_bosses: run.bosses_alive.len(),
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
        path: path.to_owned(),
        present: Path::new(path).exists(),
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
        if run.hp <= 0 {
            warnings.push(DiagnosticMessage::info(
                DiagnosticArea::Run,
                "Current run is in a loss state.",
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
        if !Path::new(required_path).exists() {
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
            GameEvent::AttackResolved { .. } | GameEvent::AttackWhiff => counters.attacks += 1,
            GameEvent::ItemTaken { .. } => counters.items_taken += 1,
            GameEvent::ItemUsed { .. } => counters.items_used += 1,
            GameEvent::Waited { .. } => counters.waits += 1,
            GameEvent::RunWon => counters.wins += 1,
            GameEvent::RunLost => counters.losses += 1,
            GameEvent::ActionRejected { .. } => counters.rejections += 1,
            _ => {}
        }
    }

    counters
}

fn format_game_event(event: &GameEvent) -> String {
    match event {
        GameEvent::HelpShown => "HelpShown".to_owned(),
        GameEvent::ActionRejected { reason } => format!("ActionRejected(reason={})", reason),
        GameEvent::LocationLooked { location_id } => {
            format!("LocationLooked(location_id={})", location_id)
        }
        GameEvent::Moved {
            from_location_id,
            to_location_id,
        } => format!("Moved(from={}, to={})", from_location_id, to_location_id),
        GameEvent::MovementBlocked {
            attempted_destination,
        } => format!(
            "MovementBlocked(attempted_destination={})",
            attempted_destination
        ),
        GameEvent::Inspected { target } => format!("Inspected(target={})", target),
        GameEvent::ItemTaken { item_id } => format!("ItemTaken(item_id={})", item_id),
        GameEvent::ItemEquipped { item_id } => format!("ItemEquipped(item_id={})", item_id),
        GameEvent::ItemUsed { item_id, effect } => match effect {
            ItemUseEffect::Healing { amount } => {
                format!("ItemUsed(item_id={}, effect=Healing({}))", item_id, amount)
            }
            ItemUseEffect::NoEffect => {
                format!("ItemUsed(item_id={}, effect=NoEffect)", item_id)
            }
        },
        GameEvent::AttackResolved {
            target_id,
            target_kind,
            damage,
            defeated,
        } => format!(
            "AttackResolved(target_id={}, kind={:?}, damage={}, defeated={})",
            target_id, target_kind, damage, defeated
        ),
        GameEvent::DamageTaken {
            amount,
            remaining_hp,
        } => format!(
            "DamageTaken(amount={}, remaining_hp={})",
            amount, remaining_hp
        ),
        GameEvent::AttackWhiff => "AttackWhiff".to_owned(),
        GameEvent::Waited { location_id } => format!("Waited(location_id={})", location_id),
        GameEvent::ObjectiveCompleted { objective_id } => {
            format!("ObjectiveCompleted(objective_id={})", objective_id)
        }
        GameEvent::RunWon => "RunWon".to_owned(),
        GameEvent::RunLost => "RunLost".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::{
        datapack_schema_version, discover_datapacks, load_datapack_bundle_by_folder,
    };
    use crate::game::actions::GameEvent;
    use crate::game::generate_new_run;

    use super::{DiagnosticArea, DiagnosticSeverity, build_diagnostic_report};

    #[test]
    fn diagnostic_report_summarizes_active_bundle_and_run() {
        let datapacks = discover_datapacks();
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let run = generate_new_run(&bundle).state;
        let events = vec![
            GameEvent::Moved {
                from_location_id: "front_verandah".to_owned(),
                to_location_id: "kitchen".to_owned(),
            },
            GameEvent::ItemTaken {
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
        assert_eq!(report.valid_datapacks, 1);
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
}
