use crate::data::datapacks::{
    DatapackBundle, LocationTemplate, MediaReferences, ResolvedMediaAssetReference,
    resolve_media_assets,
};
use crate::game::actions::EncounterKind;
use crate::game::{GameEvent, RunState};

const DATAPACK_IMAGE_FALLBACK_CANDIDATES: &[&str] = &[
    "media/images/fallbacks/placeholder.png",
    "media/images/fallbacks/scenario_default.png",
];
const ENGINE_IMAGE_FALLBACK_PATH: &str = "engine://chatty_quest_placeholder_image";

#[derive(Clone)]
pub struct MediaCue {
    pub label: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct MediaSelection {
    pub label: String,
    pub detail: String,
    pub narrator_brief: Option<String>,
    pub image_asset: Option<MediaAssetStatus>,
    pub motion_asset: Option<MediaAssetStatus>,
    pub audio_asset: Option<MediaAssetStatus>,
    pub display_role: Option<String>,
    pub used_datapack_fallback: bool,
    pub used_engine_fallback: bool,
}

#[derive(Clone)]
pub struct MediaAssetStatus {
    pub relative_path: String,
    pub resolved_path: String,
    pub present: bool,
}

#[derive(Clone)]
pub struct LocationTileImageResolution {
    pub image_asset: MediaAssetStatus,
    pub using_datapack_fallback: bool,
    pub using_engine_fallback: bool,
}

#[derive(Clone)]
pub struct MediaPanelState {
    pub title: String,
    pub subtitle: String,
    pub narrator_brief: Option<String>,
    pub selected_image: Option<MediaAssetStatus>,
    pub selected_motion: Option<MediaAssetStatus>,
    pub selected_audio: Option<MediaAssetStatus>,
    pub selected_display_role: Option<String>,
    pub has_missing_media: bool,
    pub used_datapack_fallback: bool,
    pub used_engine_fallback: bool,
    pub placeholder_message: String,
    pub world_tone: Option<String>,
    pub boundary_rule: Option<String>,
    pub current_location_name: Option<String>,
    pub current_location_description: Option<String>,
    pub active_cues: Vec<MediaCue>,
    pub encounter_snapshot: Vec<String>,
    pub future_hook_keys: Vec<String>,
}

pub fn build_media_panel_state(
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
) -> MediaPanelState {
    let world_tone = current_bundle.and_then(|bundle| bundle.world_tone.clone());
    let boundary_rule = current_run.and_then(|run| run.boundary_response.clone());
    let current_location_name = current_bundle.zip(current_run).and_then(|(bundle, run)| {
        bundle
            .locations
            .iter()
            .find(|location| location.id == run.current_location_id)
            .map(|location| location.name.clone())
    });
    let current_location_description = current_bundle.zip(current_run).and_then(|(bundle, run)| {
        bundle
            .locations
            .iter()
            .find(|location| location.id == run.current_location_id)
            .map(|location| location.description.clone())
    });

    let selection = select_media_focus(current_bundle, current_run, recent_events);
    let active_cues = build_active_cues(current_bundle, current_run, recent_events);
    let future_hook_keys = build_future_hook_keys(current_run, recent_events);
    let encounter_snapshot = build_encounter_snapshot(current_bundle, current_run);

    let (title, subtitle) = selection
        .as_ref()
        .map(|selection| (selection.label.clone(), selection.detail.clone()))
        .or_else(|| {
            active_cues
                .first()
                .map(|cue| (cue.label.clone(), cue.detail.clone()))
        })
        .unwrap_or_else(|| {
            (
                "Ambient media lane".to_owned(),
                "No strong event cue yet. Future assets can idle on location or scenario tone."
                    .to_owned(),
            )
        });

    MediaPanelState {
        title,
        subtitle,
        narrator_brief: selection
            .as_ref()
            .and_then(|selection| selection.narrator_brief.clone()),
        selected_image: selection
            .as_ref()
            .and_then(|selection| selection.image_asset.clone()),
        selected_motion: selection
            .as_ref()
            .and_then(|selection| selection.motion_asset.clone()),
        selected_audio: selection
            .as_ref()
            .and_then(|selection| selection.audio_asset.clone()),
        selected_display_role: selection
            .as_ref()
            .and_then(|selection| selection.display_role.clone()),
        has_missing_media: selection.as_ref().is_some_and(selection_has_missing_media),
        used_datapack_fallback: selection
            .as_ref()
            .is_some_and(|selection| selection.used_datapack_fallback),
        used_engine_fallback: selection
            .as_ref()
            .is_some_and(|selection| selection.used_engine_fallback),
        placeholder_message: build_placeholder_message(selection.as_ref()),
        world_tone,
        boundary_rule,
        current_location_name,
        current_location_description,
        active_cues,
        encounter_snapshot,
        future_hook_keys,
    }
}

fn select_media_focus(
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
) -> Option<MediaSelection> {
    let (Some(bundle), Some(run)) = (current_bundle, current_run) else {
        return None;
    };

    let current_location = bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id);

    for event in recent_events.iter().rev() {
        match event {
            GameEvent::Moved { to_location_id, .. }
            | GameEvent::LocationLooked {
                location_id: to_location_id,
            } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *to_location_id)
                {
                    return Some(media_selection_for_location_event(
                        bundle,
                        run,
                        location,
                        "Location focus",
                    ));
                }
            }
            GameEvent::Inspected { target }
            | GameEvent::ItemTaken { item_id: target }
            | GameEvent::ItemEquipped { item_id: target }
            | GameEvent::ItemUsed {
                item_id: target, ..
            } => {
                if let Some(item) = bundle.items.iter().find(|it| it.id == *target) {
                    return Some(media_selection_for_focus_event(
                        bundle,
                        current_location,
                        format!("Item focus: {}", item.name),
                        item.description.clone(),
                        item.narrator_brief.clone(),
                        &item.id,
                        &item.media,
                        item.media.display_role.as_deref().unwrap_or("item"),
                    ));
                }
                if let Some(enemy) = bundle.enemies.iter().find(|it| it.id == *target) {
                    return Some(media_selection_for_focus_event(
                        bundle,
                        current_location,
                        format!("Threat focus: {}", enemy.name),
                        enemy.description.clone(),
                        enemy.narrator_brief.clone(),
                        &enemy.id,
                        &enemy.media,
                        enemy.media.display_role.as_deref().unwrap_or("enemy"),
                    ));
                }
                if let Some(boss) = bundle.bosses.iter().find(|it| it.id == *target) {
                    return Some(media_selection_for_focus_event(
                        bundle,
                        current_location,
                        format!("Boss focus: {}", boss.name),
                        boss.description.clone(),
                        boss.narrator_brief.clone(),
                        &boss.id,
                        &boss.media,
                        boss.media.display_role.as_deref().unwrap_or("boss"),
                    ));
                }
            }
            GameEvent::AttackResolved {
                target_id,
                target_kind,
                defeated,
                ..
            } => match target_kind {
                EncounterKind::Enemy => {
                    if let Some(enemy) = bundle.enemies.iter().find(|it| it.id == *target_id) {
                        let beat = if *defeated {
                            "Defeat focus"
                        } else {
                            "Combat focus"
                        };
                        return Some(media_selection_for_focus_event(
                            bundle,
                            current_location,
                            format!("{}: {}", beat, enemy.name),
                            enemy.description.clone(),
                            enemy.narrator_brief.clone(),
                            &enemy.id,
                            &enemy.media,
                            enemy.media.display_role.as_deref().unwrap_or("enemy"),
                        ));
                    }
                }
                EncounterKind::Boss => {
                    if let Some(boss) = bundle.bosses.iter().find(|it| it.id == *target_id) {
                        let beat = if *defeated {
                            "Boss defeat focus"
                        } else {
                            "Boss combat focus"
                        };
                        return Some(media_selection_for_focus_event(
                            bundle,
                            current_location,
                            format!("{}: {}", beat, boss.name),
                            boss.description.clone(),
                            boss.narrator_brief.clone(),
                            &boss.id,
                            &boss.media,
                            boss.media.display_role.as_deref().unwrap_or("boss"),
                        ));
                    }
                }
            },
            GameEvent::Waited { location_id } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *location_id) {
                    return Some(media_selection_for_location_event(
                        bundle,
                        run,
                        location,
                        "Ambient focus",
                    ));
                }
            }
            GameEvent::HelpShown
            | GameEvent::ActionRejected { .. }
            | GameEvent::MovementBlocked { .. }
            | GameEvent::DamageTaken { .. }
            | GameEvent::AttackWhiff
            | GameEvent::ObjectiveCompleted { .. }
            | GameEvent::RunWon
            | GameEvent::RunLost => {}
        }
    }

    bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id)
        .map(|location| media_selection_for_location_event(bundle, run, location, "Location focus"))
}

fn build_active_cues(
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
) -> Vec<MediaCue> {
    let Some(bundle) = current_bundle else {
        return vec![MediaCue {
            label: "No active bundle".to_owned(),
            detail: "Media hooks stay dormant until a validated datapack bundle is loaded."
                .to_owned(),
        }];
    };

    let mut cues = Vec::new();

    for event in recent_events.iter().rev().take(6) {
        match event {
            GameEvent::Moved { to_location_id, .. } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *to_location_id)
                {
                    cues.push(MediaCue {
                        label: format!("Location shift: {}", location.name),
                        detail: location.description.clone(),
                    });
                }
            }
            GameEvent::LocationLooked { location_id } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *location_id) {
                    cues.push(MediaCue {
                        label: format!("Inspection focus: {}", location.name),
                        detail: location.description.clone(),
                    });
                }
            }
            GameEvent::Inspected { target } => {
                if let Some(item) = bundle.items.iter().find(|it| it.id == *target) {
                    cues.push(MediaCue {
                        label: format!("Item close-up: {}", item.name),
                        detail: item.description.clone(),
                    });
                } else if let Some(enemy) = bundle.enemies.iter().find(|it| it.id == *target) {
                    cues.push(MediaCue {
                        label: format!("Threat close-up: {}", enemy.name),
                        detail: enemy.description.clone(),
                    });
                } else if let Some(boss) = bundle.bosses.iter().find(|it| it.id == *target) {
                    cues.push(MediaCue {
                        label: format!("Boss close-up: {}", boss.name),
                        detail: boss.description.clone(),
                    });
                }
            }
            GameEvent::ItemTaken { item_id } => {
                if let Some(item) = bundle.items.iter().find(|it| it.id == *item_id) {
                    cues.push(MediaCue {
                        label: format!("Pickup beat: {}", item.name),
                        detail: item.description.clone(),
                    });
                }
            }
            GameEvent::ItemEquipped { item_id } => {
                if let Some(item) = bundle.items.iter().find(|it| it.id == *item_id) {
                    cues.push(MediaCue {
                        label: format!("Equipped: {}", item.name),
                        detail:
                            "A future media hook could swap in an equipped-item portrait or stance."
                                .to_owned(),
                    });
                }
            }
            GameEvent::ItemUsed { item_id, .. } => {
                if let Some(item) = bundle.items.iter().find(|it| it.id == *item_id) {
                    cues.push(MediaCue {
                        label: format!("Item effect: {}", item.name),
                        detail:
                            "A future media hook could surface a consumable effect splash here."
                                .to_owned(),
                    });
                }
            }
            GameEvent::AttackResolved {
                target_id,
                target_kind,
                defeated,
                ..
            } => {
                let (name, description) = lookup_encounter(bundle, target_id, target_kind);
                let beat = if *defeated {
                    "Defeat beat"
                } else {
                    "Combat beat"
                };
                cues.push(MediaCue {
                    label: format!("{}: {}", beat, name),
                    detail: description,
                });
            }
            GameEvent::DamageTaken {
                amount,
                remaining_hp,
            } => {
                cues.push(MediaCue {
                    label: "Player damage".to_owned(),
                    detail: format!(
                        "A future media hook could pulse the UI or swap a damaged portrait. HP now {} after taking {} damage.",
                        remaining_hp, amount
                    ),
                });
            }
            GameEvent::MovementBlocked {
                attempted_destination,
            } => {
                cues.push(MediaCue {
                    label: "Boundary block".to_owned(),
                    detail: format!(
                        "Movement toward '{}' was blocked. This is a good future seam for a denial image or short video sting.",
                        attempted_destination
                    ),
                });
            }
            GameEvent::ObjectiveCompleted { objective_id } => {
                cues.push(MediaCue {
                    label: "Objective complete".to_owned(),
                    detail: format!(
                        "Objective '{}' completed. A future media hook could surface a victory card here.",
                        objective_id
                    ),
                });
            }
            GameEvent::RunWon => {
                cues.push(MediaCue {
                    label: "Run won".to_owned(),
                    detail:
                        "A future media hook could surface a scenario-clear banner or end card."
                            .to_owned(),
                });
            }
            GameEvent::RunLost => {
                cues.push(MediaCue {
                    label: "Run lost".to_owned(),
                    detail:
                        "A future media hook could surface a defeat banner or failure vignette."
                            .to_owned(),
                });
            }
            GameEvent::Waited { location_id } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *location_id) {
                    cues.push(MediaCue {
                        label: format!("Idle ambience: {}", location.name),
                        detail: location.description.clone(),
                    });
                }
            }
            GameEvent::ActionRejected { reason } => {
                cues.push(MediaCue {
                    label: "Rejected action".to_owned(),
                    detail: format!(
                        "No media state change required, but tooling can still surface this reason: {}",
                        reason
                    ),
                });
            }
            GameEvent::HelpShown | GameEvent::AttackWhiff => {}
        }
    }

    if cues.is_empty()
        && let (Some(bundle), Some(run)) = (current_bundle, current_run)
        && let Some(location) = bundle
            .locations
            .iter()
            .find(|location| location.id == run.current_location_id)
    {
        cues.push(MediaCue {
            label: format!("Ambient location: {}", location.name),
            detail: location.description.clone(),
        });
    }

    cues
}

fn build_future_hook_keys(
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(run) = current_run {
        keys.push(format!("location:{}", run.current_location_id));
        if run.active_objective.completed {
            keys.push("run:won".to_owned());
        } else if run.hp <= 0 {
            keys.push("run:lost".to_owned());
        } else {
            keys.push(format!("objective:{}", run.active_objective.id));
        }
    }

    if let Some(event) = recent_events.last() {
        match event {
            GameEvent::Moved { to_location_id, .. } => {
                keys.push(format!("event:moved:{}", to_location_id))
            }
            GameEvent::LocationLooked { location_id } => {
                keys.push(format!("event:look:{}", location_id))
            }
            GameEvent::Inspected { target } => keys.push(format!("event:inspect:{}", target)),
            GameEvent::ItemTaken { item_id } => keys.push(format!("event:item_taken:{}", item_id)),
            GameEvent::ItemEquipped { item_id } => {
                keys.push(format!("event:item_equipped:{}", item_id))
            }
            GameEvent::ItemUsed { item_id, .. } => {
                keys.push(format!("event:item_used:{}", item_id))
            }
            GameEvent::AttackResolved {
                target_id,
                target_kind,
                defeated,
                ..
            } => {
                let kind = match target_kind {
                    EncounterKind::Enemy => "enemy",
                    EncounterKind::Boss => "boss",
                };
                keys.push(format!("event:attack:{}:{}", kind, target_id));
                if *defeated {
                    keys.push(format!("event:defeated:{}:{}", kind, target_id));
                }
            }
            GameEvent::DamageTaken { .. } => keys.push("event:player_damaged".to_owned()),
            GameEvent::MovementBlocked { .. } => keys.push("event:boundary_blocked".to_owned()),
            GameEvent::ObjectiveCompleted { objective_id } => {
                keys.push(format!("event:objective_complete:{}", objective_id))
            }
            GameEvent::RunWon => keys.push("event:run_won".to_owned()),
            GameEvent::RunLost => keys.push("event:run_lost".to_owned()),
            GameEvent::Waited { location_id } => keys.push(format!("event:wait:{}", location_id)),
            GameEvent::ActionRejected { .. } => keys.push("event:action_rejected".to_owned()),
            GameEvent::HelpShown | GameEvent::AttackWhiff => {}
        }
    }

    keys
}

fn build_encounter_snapshot(
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
) -> Vec<String> {
    let (Some(bundle), Some(run)) = (current_bundle, current_run) else {
        return Vec::new();
    };

    let mut lines = Vec::new();

    for enemy in &bundle.enemies {
        let status = if run.enemies_alive.contains(&enemy.id) {
            "alive"
        } else {
            "defeated"
        };
        lines.push(format!(
            "Enemy: {} [{}] | {}",
            enemy.name,
            status,
            enemy.tags.join(", ")
        ));
    }

    for boss in &bundle.bosses {
        let status = if run.bosses_alive.contains(&boss.id) {
            "alive"
        } else {
            "defeated"
        };
        lines.push(format!(
            "Boss: {} [{}] | {}",
            boss.name,
            status,
            boss.tags.join(", ")
        ));
    }

    lines
}

fn media_selection_for_location_event(
    bundle: &DatapackBundle,
    run: &RunState,
    location: &LocationTemplate,
    prefix: &str,
) -> MediaSelection {
    let context_target = first_live_location_threat(bundle, run, location);
    let resolution = resolve_media_stack(
        bundle,
        Some(location),
        context_target,
        ResolutionMode::LocationFirst,
    );

    MediaSelection {
        label: format!("{}: {}", prefix, location.name),
        detail: location.description.clone(),
        narrator_brief: location.narrator_brief.clone(),
        image_asset: resolution.image_asset,
        motion_asset: resolution.motion_asset,
        audio_asset: resolution.audio_asset,
        display_role: Some("location".to_owned()),
        used_datapack_fallback: resolution.used_datapack_fallback,
        used_engine_fallback: resolution.used_engine_fallback,
    }
}

fn media_selection_for_focus_event(
    bundle: &DatapackBundle,
    current_location: Option<&LocationTemplate>,
    label: String,
    detail: String,
    narrator_brief: Option<String>,
    focus_id: &str,
    media: &MediaReferences,
    display_role: &str,
) -> MediaSelection {
    let resolution = resolve_media_stack(
        bundle,
        current_location,
        Some((focus_id, media)),
        ResolutionMode::FocusFirst,
    );

    MediaSelection {
        label,
        detail,
        narrator_brief,
        image_asset: resolution.image_asset,
        motion_asset: resolution.motion_asset,
        audio_asset: resolution.audio_asset,
        display_role: Some(display_role.to_owned()),
        used_datapack_fallback: resolution.used_datapack_fallback,
        used_engine_fallback: resolution.used_engine_fallback,
    }
}

struct ResolvedMediaStack {
    image_asset: Option<MediaAssetStatus>,
    motion_asset: Option<MediaAssetStatus>,
    audio_asset: Option<MediaAssetStatus>,
    used_datapack_fallback: bool,
    used_engine_fallback: bool,
}

#[derive(Clone, Copy)]
enum ResolutionMode {
    LocationFirst,
    FocusFirst,
}

fn resolve_media_stack(
    bundle: &DatapackBundle,
    current_location: Option<&LocationTemplate>,
    focus: Option<(&str, &MediaReferences)>,
    mode: ResolutionMode,
) -> ResolvedMediaStack {
    let location_assets =
        current_location.map(|location| resolve_media_assets(bundle, &location.media));
    let focus_assets = focus.map(|(_, media)| resolve_media_assets(bundle, media));
    let override_prefix = focus.and_then(|(focus_id, _)| {
        current_location.map(|location| format!("{}__{}", focus_id, location.id))
    });

    let override_image = override_prefix
        .as_deref()
        .and_then(|prefix| resolve_override_asset(bundle, "images", prefix));
    let override_motion = override_prefix
        .as_deref()
        .and_then(|prefix| resolve_override_asset(bundle, "video", prefix));
    let override_audio = override_prefix
        .as_deref()
        .and_then(|prefix| resolve_override_asset(bundle, "audio", prefix));

    let focus_image = focus_assets
        .as_ref()
        .and_then(|assets| find_resolved_asset(assets, &["image"]));
    let focus_motion = focus_assets
        .as_ref()
        .and_then(|assets| find_resolved_asset(assets, &["gif", "video"]));
    let focus_audio = focus_assets
        .as_ref()
        .and_then(|assets| find_resolved_asset(assets, &["audio"]));

    let location_image = location_assets
        .as_ref()
        .and_then(|assets| find_resolved_asset(assets, &["image"]));
    let location_motion = location_assets
        .as_ref()
        .and_then(|assets| find_resolved_asset(assets, &["gif", "video"]));
    let location_audio = location_assets
        .as_ref()
        .and_then(|assets| find_resolved_asset(assets, &["audio"]));

    let image_asset = match mode {
        ResolutionMode::LocationFirst => override_image
            .or(location_image)
            .or_else(|| resolve_datapack_image_fallback(bundle))
            .or_else(engine_image_fallback),
        ResolutionMode::FocusFirst => override_image
            .or(focus_image)
            .or(location_image)
            .or_else(|| resolve_datapack_image_fallback(bundle))
            .or_else(engine_image_fallback),
    };
    let motion_asset = match mode {
        ResolutionMode::LocationFirst => override_motion.or(location_motion),
        ResolutionMode::FocusFirst => override_motion.or(focus_motion).or(location_motion),
    };
    let audio_asset = match mode {
        ResolutionMode::LocationFirst => override_audio.or(location_audio),
        ResolutionMode::FocusFirst => override_audio.or(focus_audio).or(location_audio),
    };

    let used_datapack_fallback = image_asset.as_ref().is_some_and(|asset| {
        DATAPACK_IMAGE_FALLBACK_CANDIDATES.contains(&asset.relative_path.as_str())
    });
    let used_engine_fallback = image_asset
        .as_ref()
        .is_some_and(|asset| asset.resolved_path == ENGINE_IMAGE_FALLBACK_PATH);

    ResolvedMediaStack {
        image_asset,
        motion_asset,
        audio_asset,
        used_datapack_fallback,
        used_engine_fallback,
    }
}

fn first_live_location_threat<'a>(
    bundle: &'a DatapackBundle,
    run: &RunState,
    location: &'a LocationTemplate,
) -> Option<(&'a str, &'a MediaReferences)> {
    for boss_id in &location.bosses {
        if run.bosses_alive.contains(boss_id)
            && let Some(boss) = bundle.bosses.iter().find(|boss| boss.id == *boss_id)
        {
            return Some((boss.id.as_str(), &boss.media));
        }
    }

    for enemy_id in &location.enemies {
        if run.enemies_alive.contains(enemy_id)
            && let Some(enemy) = bundle.enemies.iter().find(|enemy| enemy.id == *enemy_id)
        {
            return Some((enemy.id.as_str(), &enemy.media));
        }
    }

    None
}

fn find_resolved_asset(
    assets: &[ResolvedMediaAssetReference],
    field_names: &[&str],
) -> Option<MediaAssetStatus> {
    field_names.iter().find_map(|field_name| {
        assets
            .iter()
            .find(|asset| asset.field_name == *field_name && asset.present)
            .map(media_asset_status_from_reference)
    })
}

fn resolve_override_asset(
    bundle: &DatapackBundle,
    media_root: &str,
    prefix: &str,
) -> Option<MediaAssetStatus> {
    let extensions: &[&str] = match media_root {
        "images" => &["png", "jpg", "jpeg", "webp"],
        "video" => &["gif", "mp4", "webm"],
        "audio" => &["ogg", "wav", "mp3"],
        _ => return None,
    };

    for extension in extensions {
        let relative_path = format!("media/{}/overrides/{}.{}", media_root, prefix, extension);
        let references = resolve_media_assets(
            bundle,
            &MediaReferences {
                image: (media_root == "images").then(|| relative_path.clone()),
                gif: (media_root == "video" && *extension == "gif").then(|| relative_path.clone()),
                video: (media_root == "video" && *extension != "gif")
                    .then(|| relative_path.clone()),
                audio: (media_root == "audio").then(|| relative_path.clone()),
                display_role: None,
            },
        );

        if let Some(asset) = references.first().filter(|asset| asset.present) {
            return Some(media_asset_status_from_reference(asset));
        }
    }

    None
}

fn resolve_datapack_image_fallback(bundle: &DatapackBundle) -> Option<MediaAssetStatus> {
    for relative_path in DATAPACK_IMAGE_FALLBACK_CANDIDATES {
        let references = resolve_media_assets(
            bundle,
            &MediaReferences {
                image: Some((*relative_path).to_owned()),
                gif: None,
                video: None,
                audio: None,
                display_role: None,
            },
        );

        if let Some(asset) = references.first().filter(|asset| asset.present) {
            return Some(media_asset_status_from_reference(asset));
        }
    }

    None
}

fn engine_image_fallback() -> Option<MediaAssetStatus> {
    Some(MediaAssetStatus {
        relative_path: "engine fallback".to_owned(),
        resolved_path: ENGINE_IMAGE_FALLBACK_PATH.to_owned(),
        present: true,
    })
}

pub fn resolve_location_tile_image(
    bundle: &DatapackBundle,
    location: &LocationTemplate,
) -> LocationTileImageResolution {
    resolve_primary_image(bundle, &location.media)
}

pub fn resolve_primary_image(
    bundle: &DatapackBundle,
    media: &MediaReferences,
) -> LocationTileImageResolution {
    let resolved_assets = resolve_media_assets(bundle, media);
    let image_asset = find_resolved_asset(&resolved_assets, &["image"])
        .or_else(|| resolve_datapack_image_fallback(bundle))
        .or_else(engine_image_fallback)
        .expect("engine fallback must always exist");

    let using_datapack_fallback =
        DATAPACK_IMAGE_FALLBACK_CANDIDATES.contains(&image_asset.relative_path.as_str());
    let using_engine_fallback = image_asset.resolved_path == ENGINE_IMAGE_FALLBACK_PATH;

    LocationTileImageResolution {
        image_asset,
        using_datapack_fallback,
        using_engine_fallback,
    }
}

fn media_asset_status_from_reference(reference: &ResolvedMediaAssetReference) -> MediaAssetStatus {
    MediaAssetStatus {
        relative_path: reference.relative_path.clone(),
        resolved_path: reference.resolved_path.clone(),
        present: reference.present,
    }
}

fn selection_has_missing_media(selection: &MediaSelection) -> bool {
    [
        selection.image_asset.as_ref(),
        selection.motion_asset.as_ref(),
        selection.audio_asset.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|asset| !asset.present)
}

fn build_placeholder_message(selection: Option<&MediaSelection>) -> String {
    let Some(selection) = selection else {
        return "No focused media selection yet. The panel is idling on world and event cues."
            .to_owned();
    };

    let referenced_assets = [
        selection.image_asset.as_ref(),
        selection.motion_asset.as_ref(),
        selection.audio_asset.as_ref(),
    ]
    .into_iter()
    .flatten()
    .count();

    if selection.used_engine_fallback {
        "No pack-owned image could be resolved, so the engine fallback placeholder is active."
            .to_owned()
    } else if selection.used_datapack_fallback {
        "Focused media fell back to the datapack placeholder image.".to_owned()
    } else if referenced_assets == 0 {
        "This focus has no media references yet. A placeholder can hold until art is added."
            .to_owned()
    } else if selection_has_missing_media(selection) {
        "Some media slots are referenced but the files are not present yet. Diagnostics has the exact missing paths."
            .to_owned()
    } else {
        "Referenced media files are present and ready for future rendering hooks.".to_owned()
    }
}

fn lookup_encounter(
    bundle: &DatapackBundle,
    target_id: &str,
    target_kind: &EncounterKind,
) -> (String, String) {
    match target_kind {
        EncounterKind::Enemy => bundle
            .enemies
            .iter()
            .find(|it| it.id == target_id)
            .map(|it| (it.name.clone(), it.description.clone()))
            .unwrap_or_else(|| (target_id.to_owned(), "Unknown enemy target.".to_owned())),
        EncounterKind::Boss => bundle
            .bosses
            .iter()
            .find(|it| it.id == target_id)
            .map(|it| (it.name.clone(), it.description.clone()))
            .unwrap_or_else(|| (target_id.to_owned(), "Unknown boss target.".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{EncounterKind, GameEvent};
    use crate::game::generation::generate_new_run;

    use super::build_media_panel_state;

    #[test]
    fn media_panel_defaults_to_location_focus_for_fresh_run() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let state = generate_new_run(&bundle).state;

        let media = build_media_panel_state(Some(&bundle), Some(&state), &[]);

        assert!(media.title.contains("Location focus"));
        assert_eq!(media.selected_display_role.as_deref(), Some("location"));
        assert_eq!(
            media.current_location_name.as_deref(),
            Some("Front Verandah")
        );
    }

    #[test]
    fn media_panel_shifts_focus_to_reducer_confirmed_item_event() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let state = generate_new_run(&bundle).state;
        let events = vec![GameEvent::ItemTaken {
            item_id: "medkit".to_owned(),
        }];

        let media = build_media_panel_state(Some(&bundle), Some(&state), &events);

        assert!(media.title.contains("Item focus: Medkit"));
        assert_eq!(media.selected_display_role.as_deref(), Some("item"));
        assert!(
            media
                .future_hook_keys
                .iter()
                .any(|key| key == "event:item_taken:medkit")
        );
    }

    #[test]
    fn media_panel_shifts_focus_to_boss_combat_event() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        let events = vec![GameEvent::AttackResolved {
            target_id: "brute_in_garage".to_owned(),
            target_kind: EncounterKind::Boss,
            damage: 3,
            defeated: false,
        }];

        let media = build_media_panel_state(Some(&bundle), Some(&state), &events);

        assert!(media.title.contains("Boss combat focus: Garage Brute"));
        assert_eq!(media.selected_display_role.as_deref(), Some("boss"));
        assert!(
            media
                .encounter_snapshot
                .iter()
                .any(|line| line.contains("Garage Brute"))
        );
    }
}
