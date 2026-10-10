use crate::collection::load_player_templates_for_datapack;
use crate::data::datapacks::{
    DatapackBundle, LocationTemplate, MediaReferences, ResolvedMediaAssetReference,
    resolve_media_assets,
};
use crate::game::actions::EncounterKind;
use crate::game::{GameEvent, RunState, enemy_template_id, find_boss, find_enemy};
use crate::media_pool::{PlayerMediaKind, load_player_media_pool, select_pool_asset};

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
    /// Runtime entities currently observable at the focused location. These are host state,
    /// not narrator claims, and give the media lane a stable place to attach future portraits.
    pub observed_entity_rows: Vec<String>,
    /// Focusable host-confirmed entities at the current location. Selecting one changes only
    /// presentation focus; it does not issue a command or mutate game state.
    pub observed_entities: Vec<ObservableEntity>,
    /// An in-memory UI choice among the currently observable entities.
    pub focused_entity_id: Option<String>,
}

#[derive(Clone)]
pub struct ObservableEntity {
    pub id: String,
    pub label: String,
    pub detail: String,
}

#[cfg(test)]
fn build_media_panel_state(
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
) -> MediaPanelState {
    build_media_panel_state_with_focus(current_bundle, current_run, recent_events, None)
}

pub fn build_media_panel_state_with_focus(
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
    requested_focus_id: Option<&str>,
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

    let observed_entities = current_bundle
        .zip(current_run)
        .map(|(bundle, run)| build_observable_entities(bundle, run))
        .unwrap_or_default();
    let focused_entity_id = requested_focus_id
        .filter(|id| observed_entities.iter().any(|entity| entity.id == *id))
        .map(str::to_owned);
    let selection = current_bundle
        .zip(current_run)
        .and_then(|(bundle, run)| {
            focused_entity_id
                .as_deref()
                .and_then(|id| explicit_entity_media_selection(bundle, run, id))
        })
        .or_else(|| select_media_focus(current_bundle, current_run, recent_events));
    let active_cues = build_active_cues(current_bundle, current_run, recent_events);
    let future_hook_keys = build_future_hook_keys(current_bundle, current_run, recent_events);
    let encounter_snapshot = build_encounter_snapshot(current_bundle, current_run);
    let observed_entity_rows = observed_entities
        .iter()
        .map(|entity| format!("{} — {}", entity.label, entity.detail))
        .collect();

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
        observed_entity_rows,
        observed_entities,
        focused_entity_id,
    }
}

fn build_observable_entities(bundle: &DatapackBundle, run: &RunState) -> Vec<ObservableEntity> {
    let location_id = &run.current_location_id;
    let mut entities = run
        .runtime_actor_ids_at(location_id)
        .into_iter()
        .filter(|actor_id| actor_id != &run.player_entity_id)
        .filter_map(|actor_id| run.runtime_actor(&actor_id))
        .map(|actor| {
            let mut detail = format_runtime_actor_observation(run, &actor);
            let owner = crate::game::state::ItemOwner::Entity(actor.entity_id.clone());
            if run.accepts_item_transfer(&owner) {
                let contents = run
                    .entity_inventory(&owner)
                    .map(|items| {
                        if items.is_empty() {
                            "empty".to_owned()
                        } else {
                            items
                                .iter()
                                .map(|item| item.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    })
                    .unwrap_or_else(|| "unavailable".to_owned());
                detail.push_str(&format!(" | inventory: {contents}"));
            }
            let abilities = run
                .ability_grants
                .iter()
                .filter(|grant| run.ability_grant_is_accessible_to(grant, &actor.entity_id))
                .map(|grant| {
                    let name = bundle
                        .abilities
                        .iter()
                        .find(|ability| ability.id == grant.template_id)
                        .map(|ability| ability.name.as_str())
                        .unwrap_or(grant.template_id.as_str());
                    let uses = grant
                        .remaining_uses
                        .map_or_else(|| "infinite".to_owned(), |uses| uses.to_string());
                    let cooldown = crate::game::ability_grant_cooldown_remaining(run, grant);
                    let readiness = crate::game::ability_grant_unavailability_reason(
                        run,
                        bundle,
                        grant,
                        &actor.entity_id,
                    )
                    .map_or_else(
                        || "ready".to_owned(),
                        |reason| format!("unavailable: {reason}"),
                    );
                    let cooldown = if cooldown == 0 {
                        "ready".to_owned()
                    } else {
                        format!("cooldown: {cooldown} world turn(s)")
                    };
                    format!("{name} ({uses}; {cooldown}; {readiness})")
                })
                .collect::<Vec<_>>();
            if !abilities.is_empty() {
                detail.push_str(&format!(" | abilities: {}", abilities.join(", ")));
            }
            ObservableEntity {
                id: observable_entity_id(run, &actor),
                label: format!("[{}] {}", actor.presentation_kind, actor.name),
                detail,
            }
        })
        .collect::<Vec<_>>();

    let enemy_ids = run
        .location_enemies
        .get(location_id)
        .cloned()
        .unwrap_or_else(|| {
            bundle
                .locations
                .iter()
                .find(|location| location.id == *location_id)
                .map(|location| location.enemies.clone())
                .unwrap_or_default()
        });
    for enemy_id in enemy_ids {
        if !run.enemies_alive.contains(&enemy_id) {
            continue;
        }
        if let Some(enemy) = run.enemy_instances.get(&enemy_id) {
            entities.push(ObservableEntity {
                id: enemy.instance_id.clone(),
                label: format!("[Enemy] {}", enemy.name),
                detail: format!(
                    "{} | runtime threat | hp {}/{} | damage {} | tags: {}",
                    enemy.description,
                    enemy.current_hp,
                    enemy.max_hp,
                    enemy.damage,
                    enemy.tags.join(", ")
                ),
            });
        } else if let Some(enemy) = find_enemy(bundle, &enemy_id) {
            entities.push(ObservableEntity {
                id: enemy.id.clone(),
                label: format!("[Enemy] {}", enemy.name),
                detail: enemy.description.clone(),
            });
        }
    }

    let boss_ids = run
        .location_bosses
        .get(location_id)
        .cloned()
        .unwrap_or_else(|| {
            bundle
                .locations
                .iter()
                .find(|location| location.id == *location_id)
                .map(|location| location.bosses.clone())
                .unwrap_or_default()
        });
    for boss_id in boss_ids {
        if !run.bosses_alive.contains(&boss_id) {
            continue;
        }
        if let Some(boss) = run.boss_instances.get(&boss_id) {
            entities.push(ObservableEntity {
                id: boss.instance_id.clone(),
                label: format!("[Boss] {}", boss.name),
                detail: format!(
                    "{} | runtime boss | hp {}/{} | damage {} | tags: {}",
                    boss.description,
                    boss.current_hp,
                    boss.max_hp,
                    boss.damage,
                    boss.tags.join(", ")
                ),
            });
        } else if let Some(boss) = bundle.bosses.iter().find(|boss| boss.id == boss_id) {
            entities.push(ObservableEntity {
                id: boss.id.clone(),
                label: format!("[Boss] {}", boss.name),
                detail: boss.description.clone(),
            });
        }
    }

    // Encounters can enter this list through both the generic runtime-actor directory and the
    // compatibility encounter buckets. Normalize each row to its stable live instance id before
    // deduplicating so those two projections cannot produce two Focus controls for one actor.
    entities.sort_by(|left, right| left.id.cmp(&right.id));
    entities.dedup_by(|left, right| left.id == right.id);
    entities.sort_by(|left, right| left.label.cmp(&right.label));
    entities
}

fn observable_entity_id(run: &RunState, actor: &crate::game::state::RuntimeActorView) -> String {
    run.enemy_instances
        .get(&actor.entity_id)
        .map(|encounter| encounter.instance_id.clone())
        .or_else(|| {
            run.boss_instances
                .get(&actor.entity_id)
                .map(|encounter| encounter.instance_id.clone())
        })
        .unwrap_or_else(|| actor.entity_id.clone())
}

fn explicit_entity_media_selection(
    bundle: &DatapackBundle,
    run: &RunState,
    entity_id: &str,
) -> Option<MediaSelection> {
    let current_location = current_location_for_run(bundle, run);
    if run.runtime_actor(entity_id).is_some() {
        return runtime_actor_media_selection(bundle, run, entity_id);
    }
    if let Some(enemy) = run.enemy_instances.get(entity_id).or_else(|| {
        run.enemy_instances
            .values()
            .find(|enemy| enemy.instance_id == entity_id)
    }) {
        // Authored encounters are keyed by their template id, while UI focus uses
        // their live instance id.  Prefer the authored contract (and its exact
        // media) when that template still exists; dynamically admitted enemies
        // intentionally fall through to the player-library/pool resolver.
        if let Some(template) = find_enemy(bundle, &enemy.template_id) {
            return Some(media_selection_for_focus_event(
                bundle,
                current_location,
                format!("Threat focus: {}", template.name),
                template.description.clone(),
                template.narrator_brief.clone(),
                &template.id,
                &template.media,
                template.media.display_role.as_deref().unwrap_or("enemy"),
            ));
        }
        return Some(player_library_entity_media_selection(
            PlayerLibraryMediaContext {
                bundle,
                current_location,
            },
            "Enemy",
            &enemy.name,
            &enemy.description,
            &enemy.tags,
            PlayerLibraryMediaIdentity {
                stable_key: &enemy.instance_id,
                datapack_id: &run.datapack_id,
                template_id: &enemy.template_id,
            },
            "enemy",
        ));
    }
    if let Some(boss) = run.boss_instances.get(entity_id).or_else(|| {
        run.boss_instances
            .values()
            .find(|boss| boss.instance_id == entity_id)
    }) {
        if let Some(template) = find_boss(bundle, &boss.template_id) {
            return Some(media_selection_for_focus_event(
                bundle,
                current_location,
                format!("Boss focus: {}", template.name),
                template.description.clone(),
                template.narrator_brief.clone(),
                &template.id,
                &template.media,
                template.media.display_role.as_deref().unwrap_or("boss"),
            ));
        }
        return Some(player_library_entity_media_selection(
            PlayerLibraryMediaContext {
                bundle,
                current_location,
            },
            "Boss",
            &boss.name,
            &boss.description,
            &boss.tags,
            PlayerLibraryMediaIdentity {
                stable_key: &boss.instance_id,
                datapack_id: &run.datapack_id,
                template_id: &boss.template_id,
            },
            "boss",
        ));
    }
    if let Some(enemy) = find_enemy(bundle, entity_id) {
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
    bundle
        .bosses
        .iter()
        .find(|boss| boss.id == entity_id)
        .map(|boss| {
            media_selection_for_focus_event(
                bundle,
                current_location,
                format!("Boss focus: {}", boss.name),
                boss.description.clone(),
                boss.narrator_brief.clone(),
                &boss.id,
                &boss.media,
                boss.media.display_role.as_deref().unwrap_or("boss"),
            )
        })
}

fn format_runtime_actor_observation(
    run: &RunState,
    actor: &crate::game::state::RuntimeActorView,
) -> String {
    let mut contract = if actor.actor.capabilities.is_empty() {
        "no optional capabilities".to_owned()
    } else {
        actor.actor.capabilities.join(", ")
    };
    if actor.actor.is_combatant() {
        contract.push_str(" | combat");
    }
    if actor.actor.has_health_contract()
        && let Some(hp) = actor.actor.current_hp
    {
        contract.push_str(&format!(
            " | hp {}/{}",
            hp,
            actor.actor.max_hp.unwrap_or(hp)
        ));
    }
    let social = run
        .social_state(&actor.entity_id)
        .map(|social| format!(" | relationship {}", social.relationship_score))
        .unwrap_or_default();
    let tags = if actor.tags.is_empty() {
        "none".to_owned()
    } else {
        actor.tags.join(", ")
    };
    let personality_tags = crate::game::social::personality_tags(&actor.tags);
    let personality = if personality_tags.is_empty() {
        String::new()
    } else {
        format!(" | personality {}", personality_tags.join(", "))
    };
    format!(
        "[{}] {} — {} | disposition {}{}{} | tags: {} | {}",
        actor.presentation_kind,
        actor.name,
        actor.description,
        format!("{:?}", actor.disposition).to_ascii_lowercase(),
        social,
        personality,
        tags,
        contract
    )
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
            GameEvent::SocialEventApplied { .. } => {}
            GameEvent::Moved { to_location_id, .. }
            | GameEvent::LocationLooked {
                location_id: to_location_id,
                ..
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
            GameEvent::Inspected { target, .. }
            | GameEvent::ItemTaken {
                item_id: target, ..
            }
            | GameEvent::ItemTransferred {
                item_id: target, ..
            }
            | GameEvent::ItemEquipped {
                item_id: target, ..
            }
            | GameEvent::ItemUsed {
                item_id: target, ..
            }
            | GameEvent::ItemConsumed {
                template_id: target,
                ..
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
                if let Some(enemy) = find_enemy(bundle, target) {
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
                if run.player_media_enabled
                    && let Some(item) = run.find_runtime_item(target)
                {
                    return Some(player_library_media_selection(bundle, run, item));
                }
            }
            GameEvent::NarratorItemAdmitted { item_id } => {
                if run.player_media_enabled
                    && let Some(item) = run.find_runtime_item(item_id)
                {
                    return Some(player_library_media_selection(bundle, run, item));
                }
            }
            GameEvent::NarratorNpcAdmitted { instance_id } => {
                if run.player_media_enabled
                    && let Some(selection) = runtime_actor_media_selection(bundle, run, instance_id)
                {
                    return Some(selection);
                }
            }
            GameEvent::NarratorEnemyAdmitted { instance_id } => {
                if run.player_media_enabled
                    && let Some(enemy) = run.enemy_instances.get(instance_id)
                {
                    return Some(player_library_entity_media_selection(
                        PlayerLibraryMediaContext {
                            bundle,
                            current_location,
                        },
                        "Enemy",
                        &enemy.name,
                        &format!("A newly admitted enemy with {} HP.", enemy.current_hp),
                        &enemy.tags,
                        PlayerLibraryMediaIdentity {
                            stable_key: &enemy.instance_id,
                            datapack_id: &run.datapack_id,
                            template_id: &enemy.template_id,
                        },
                        "enemy",
                    ));
                }
            }
            GameEvent::NarratorBossAdmitted { instance_id } => {
                if run.player_media_enabled
                    && let Some(boss) = run.boss_instances.get(instance_id)
                {
                    return Some(player_library_entity_media_selection(
                        PlayerLibraryMediaContext {
                            bundle,
                            current_location,
                        },
                        "Boss",
                        &boss.name,
                        &format!("A newly admitted boss with {} HP.", boss.current_hp),
                        &boss.tags,
                        PlayerLibraryMediaIdentity {
                            stable_key: &boss.instance_id,
                            datapack_id: &run.datapack_id,
                            template_id: &boss.template_id,
                        },
                        "boss",
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
                    if let Some(enemy) = find_enemy(bundle, target_id) {
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
                    if run.player_media_enabled
                        && let Some(enemy) = run.enemy_instances.get(target_id)
                    {
                        return Some(player_library_entity_media_selection(
                            PlayerLibraryMediaContext {
                                bundle,
                                current_location,
                            },
                            "Enemy",
                            &enemy.name,
                            &format!("A runtime enemy with {} HP.", enemy.current_hp),
                            &enemy.tags,
                            PlayerLibraryMediaIdentity {
                                stable_key: &enemy.instance_id,
                                datapack_id: &run.datapack_id,
                                template_id: &enemy.template_id,
                            },
                            "enemy",
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
                    if run.player_media_enabled
                        && let Some(boss) = run.boss_instances.get(target_id)
                    {
                        return Some(player_library_entity_media_selection(
                            PlayerLibraryMediaContext {
                                bundle,
                                current_location,
                            },
                            "Boss",
                            &boss.name,
                            &format!("A runtime boss with {} HP.", boss.current_hp),
                            &boss.tags,
                            PlayerLibraryMediaIdentity {
                                stable_key: &boss.instance_id,
                                datapack_id: &run.datapack_id,
                                template_id: &boss.template_id,
                            },
                            "boss",
                        ));
                    }
                }
                EncounterKind::Npc => {
                    if let Some(npc) = bundle.npcs.iter().find(|it| it.id == *target_id) {
                        let beat = if *defeated {
                            "NPC defeat focus"
                        } else {
                            "NPC combat focus"
                        };
                        return Some(media_selection_for_focus_event(
                            bundle,
                            current_location,
                            format!("{}: {}", beat, npc.name),
                            npc.description.clone(),
                            npc.narrator_brief.clone(),
                            &npc.id,
                            &npc.media,
                            npc.media.display_role.as_deref().unwrap_or("npc"),
                        ));
                    }
                    if run.player_media_enabled
                        && let Some(selection) =
                            runtime_actor_media_selection(bundle, run, target_id)
                    {
                        return Some(selection);
                    }
                }
                EncounterKind::Actor => {
                    if run.player_media_enabled
                        && let Some(selection) =
                            runtime_actor_media_selection(bundle, run, target_id)
                    {
                        return Some(selection);
                    }
                }
            },
            GameEvent::AbilityUsed {
                origin_id,
                target_id,
                ..
            } => {
                if run.player_media_enabled
                    && let Some(selection) = runtime_actor_media_selection(bundle, run, target_id)
                        .or_else(|| runtime_actor_media_selection(bundle, run, origin_id))
                {
                    return Some(selection);
                }
            }
            GameEvent::ActorSpawnedFromNoise {
                template_id,
                location_id,
                ..
            } => {
                if let Some(enemy) = find_enemy(bundle, template_id) {
                    let location = bundle.locations.iter().find(|it| it.id == *location_id);
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Noise spawn: {}", enemy.name),
                        enemy.description.clone(),
                        enemy.narrator_brief.clone(),
                        &enemy.id,
                        &enemy.media,
                        enemy.media.display_role.as_deref().unwrap_or("enemy"),
                    ));
                }
                if let Some(npc) = bundle.npcs.iter().find(|npc| npc.id == *template_id) {
                    let location = bundle.locations.iter().find(|it| it.id == *location_id);
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Noise spawn: {}", npc.name),
                        npc.description.clone(),
                        npc.narrator_brief.clone(),
                        &npc.id,
                        &npc.media,
                        npc.media.display_role.as_deref().unwrap_or("npc"),
                    ));
                }
                if let Some(boss) = bundle.bosses.iter().find(|boss| boss.id == *template_id) {
                    let location = bundle.locations.iter().find(|it| it.id == *location_id);
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Noise spawn: {}", boss.name),
                        boss.description.clone(),
                        boss.narrator_brief.clone(),
                        &boss.id,
                        &boss.media,
                        boss.media.display_role.as_deref().unwrap_or("boss"),
                    ));
                }
            }
            GameEvent::NoiseAttractorShifted { location_id, .. } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *location_id) {
                    return Some(media_selection_for_location_event(
                        bundle,
                        run,
                        location,
                        "Noise attractor",
                    ));
                }
            }
            GameEvent::SightAttractorAcquired {
                origin_id,
                location_id,
                ..
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    let location = bundle.locations.iter().find(|it| it.id == *location_id);
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Sight chase: {}", enemy.name),
                        enemy.description.clone(),
                        enemy.narrator_brief.clone(),
                        &enemy.id,
                        &enemy.media,
                        enemy.media.display_role.as_deref().unwrap_or("enemy"),
                    ));
                }
            }
            GameEvent::SightAttractorMissed {
                origin_id,
                location_id,
                ..
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    let location = bundle.locations.iter().find(|it| it.id == *location_id);
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Sight miss: {}", enemy.name),
                        enemy.description.clone(),
                        enemy.narrator_brief.clone(),
                        &enemy.id,
                        &enemy.media,
                        enemy.media.display_role.as_deref().unwrap_or("enemy"),
                    ));
                }
            }
            GameEvent::SightAttractorLost { origin_id, .. } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    let location = run
                        .location_enemies
                        .iter()
                        .find(|(_, enemy_ids)| enemy_ids.iter().any(|entry| entry == origin_id))
                        .and_then(|(location_id, _)| {
                            bundle.locations.iter().find(|it| it.id == *location_id)
                        });
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Sight lost: {}", enemy.name),
                        enemy.description.clone(),
                        enemy.narrator_brief.clone(),
                        &enemy.id,
                        &enemy.media,
                        enemy.media.display_role.as_deref().unwrap_or("enemy"),
                    ));
                }
            }
            GameEvent::ActorMoved {
                origin_id,
                to_location_id,
                ..
            }
            | GameEvent::ActorWaited {
                origin_id,
                location_id: to_location_id,
                ..
            }
            | GameEvent::ActorAttackedHazard {
                origin_id,
                location_id: to_location_id,
                ..
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    let location = bundle.locations.iter().find(|it| it.id == *to_location_id);
                    return Some(media_selection_for_focus_event(
                        bundle,
                        location.or(current_location),
                        format!("Spawned threat: {}", enemy.name),
                        enemy.description.clone(),
                        enemy.narrator_brief.clone(),
                        &enemy.id,
                        &enemy.media,
                        enemy.media.display_role.as_deref().unwrap_or("enemy"),
                    ));
                }
                if run.player_media_enabled
                    && let Some(selection) = runtime_actor_media_selection(bundle, run, origin_id)
                {
                    return Some(selection);
                }
            }
            GameEvent::Waited { location_id, .. } => {
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
            | GameEvent::SelectedPackFailedValidation { .. }
            | GameEvent::MovementBlocked { .. }
            | GameEvent::LocationUnlocked { .. }
            | GameEvent::LocationBarricaded { .. }
            | GameEvent::DamageTaken { .. }
            | GameEvent::HealthRestored { .. }
            | GameEvent::AttackWhiff { .. }
            | GameEvent::ObjectiveCompleted { .. }
            | GameEvent::RunWon
            | GameEvent::RunLost { .. }
            | GameEvent::StatusEffectExpired { .. }
            | GameEvent::NarratorStatusAdmitted { .. }
            | GameEvent::StatusEffectApplied { .. }
            | GameEvent::ConnectionsRevealed { .. }
            | GameEvent::AbilityCooldownSet { .. }
            | GameEvent::ProgressAwarded { .. }
            | GameEvent::SkillLevelGained { .. }
            | GameEvent::MaxHpIncreased { .. }
            | GameEvent::AbilityGranted { .. }
            | GameEvent::AbilityGrantExpired { .. } => {}
            GameEvent::NarratorTemporaryExitAdmitted { .. } => {}
        }
    }

    bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id)
        .map(|location| media_selection_for_location_event(bundle, run, location, "Location focus"))
}

fn item_media_cue(
    bundle: &DatapackBundle,
    run: Option<&RunState>,
    item_id: &str,
    authored_label: &str,
    player_library_label: &str,
) -> Option<MediaCue> {
    if let Some(item) = bundle.items.iter().find(|item| item.id == item_id) {
        return Some(MediaCue {
            label: format!("{authored_label}: {}", item.name),
            detail: item.description.clone(),
        });
    }

    run?.find_runtime_item(item_id).map(|item| MediaCue {
        label: format!("{player_library_label}: {}", item.name),
        detail: item.description.clone(),
    })
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
            GameEvent::SocialEventApplied { .. } => {}
            GameEvent::Moved { to_location_id, .. } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *to_location_id)
                {
                    cues.push(MediaCue {
                        label: format!("Location shift: {}", location.name),
                        detail: location.description.clone(),
                    });
                }
            }
            GameEvent::LocationLooked { location_id, .. } => {
                if let Some(location) = bundle.locations.iter().find(|it| it.id == *location_id) {
                    cues.push(MediaCue {
                        label: format!("Inspection focus: {}", location.name),
                        detail: location.description.clone(),
                    });
                }
            }
            GameEvent::Inspected { target, .. } => {
                if let Some(cue) = item_media_cue(
                    bundle,
                    current_run,
                    target,
                    "Item close-up",
                    "Player-library item close-up",
                ) {
                    cues.push(cue);
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
            GameEvent::ItemTaken { item_id, .. } | GameEvent::ItemTransferred { item_id, .. } => {
                if let Some(cue) = item_media_cue(
                    bundle,
                    current_run,
                    item_id,
                    "Pickup beat",
                    "Player-library transfer",
                ) {
                    cues.push(cue);
                }
            }
            GameEvent::ItemEquipped { item_id, .. } => {
                if let Some(item) = bundle
                    .items
                    .iter()
                    .find(|it| it.id == *item_id)
                    .map(|item| (item.name.clone(), true))
                    .or_else(|| {
                        current_run
                            .and_then(|run| run.find_runtime_item(item_id))
                            .map(|item| (item.name.clone(), false))
                    })
                {
                    cues.push(MediaCue {
                        label: format!(
                            "{}: {}",
                            if item.1 {
                                "Equipped"
                            } else {
                                "Player-library equipped"
                            },
                            item.0
                        ),
                        detail:
                            "A future media hook could swap in an equipped-item portrait or stance."
                                .to_owned(),
                    });
                }
            }
            GameEvent::ItemUsed { item_id, .. } => {
                if let Some(item) = bundle
                    .items
                    .iter()
                    .find(|it| it.id == *item_id)
                    .map(|item| (item.name.clone(), true))
                    .or_else(|| {
                        current_run
                            .and_then(|run| run.find_runtime_item(item_id))
                            .map(|item| (item.name.clone(), false))
                    })
                {
                    cues.push(MediaCue {
                        label: format!(
                            "{}: {}",
                            if item.1 {
                                "Item effect"
                            } else {
                                "Player-library item effect"
                            },
                            item.0
                        ),
                        detail:
                            "A future media hook could surface a consumable effect splash here."
                                .to_owned(),
                    });
                }
            }
            GameEvent::ItemConsumed { template_id, .. } => {
                if let Some(item) = bundle
                    .items
                    .iter()
                    .find(|it| it.id == *template_id)
                    .map(|item| (item.name.clone(), true))
                {
                    cues.push(MediaCue {
                        label: format!("Source studied: {}", item.0),
                        detail: "A source item was consumed to materialize permanent knowledge."
                            .to_owned(),
                    });
                }
            }
            GameEvent::AbilityUsed {
                origin_id,
                template_id,
                target_id,
                effect,
                remaining_uses,
                ..
            } => cues.push(MediaCue {
                label: format!("Ability {effect}: {template_id}"),
                detail: format!(
                    "{} used {} on {}; {} uses remain.",
                    origin_id,
                    template_id,
                    target_id,
                    remaining_uses.map_or("infinite".to_owned(), |uses| uses.to_string())
                ),
            }),
            GameEvent::NarratorItemAdmitted { item_id } => {
                if let Some(cue) = item_media_cue(
                    bundle,
                    current_run,
                    item_id,
                    "Narrator-admitted item",
                    "Narrator-admitted player-library item",
                ) {
                    cues.push(cue);
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
                target_id,
                amount,
                remaining_hp,
                ..
            } => {
                cues.push(MediaCue {
                    label: format!("Damage: {}", target_id),
                    detail: format!(
                        "A future media hook could pulse the damaged actor's UI or swap its portrait. {} has {} HP after taking {} damage.",
                        target_id, remaining_hp, amount
                    ),
                });
            }
            GameEvent::MovementBlocked {
                attempted_destination,
                ..
            } => {
                cues.push(MediaCue {
                    label: "Boundary block".to_owned(),
                    detail: format!(
                        "Movement toward '{}' was blocked. This is a good future seam for a denial image or short video sting.",
                        attempted_destination
                    ),
                });
            }
            GameEvent::LocationUnlocked {
                location_id,
                item_id,
                ..
            } => {
                cues.push(MediaCue {
                    label: "Gate unlocked".to_owned(),
                    detail: format!(
                        "Location '{}' was unlocked with '{}'. A future media hook could surface a mechanical progression beat here.",
                        location_id, item_id
                    ),
                });
            }
            GameEvent::LocationBarricaded {
                location_id,
                item_id,
                ..
            } => {
                cues.push(MediaCue {
                    label: "Approach barricaded".to_owned(),
                    detail: format!(
                        "Location '{}' was barricaded with '{}'. A future media hook could surface a secured-threshold beat here.",
                        location_id, item_id
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
            GameEvent::RunLost { .. } => {
                cues.push(MediaCue {
                    label: "Run lost".to_owned(),
                    detail:
                        "A future media hook could surface a defeat banner or failure vignette."
                            .to_owned(),
                });
            }
            GameEvent::Waited { location_id, .. } => {
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
            GameEvent::SelectedPackFailedValidation {
                folder_name,
                reason,
            } => {
                cues.push(MediaCue {
                    label: "Cartridge refused".to_owned(),
                    detail: format!(
                        "Selected cartridge '{}' failed validation: {}",
                        folder_name, reason
                    ),
                });
            }
            GameEvent::ActorSpawnedFromNoise {
                template_id,
                location_id,
                ..
            } => {
                let location_name = bundle
                    .locations
                    .iter()
                    .find(|location| location.id == *location_id)
                    .map(|location| location.name.clone())
                    .unwrap_or_else(|| location_id.clone());
                if let Some(enemy) = find_enemy(bundle, template_id) {
                    cues.push(MediaCue {
                        label: format!("Noise spawn: {}", enemy.name),
                        detail: format!(
                            "Swarming noise pulled this threat into {}.",
                            location_name
                        ),
                    });
                }
            }
            GameEvent::NoiseAttractorShifted {
                location_id,
                actor_ids,
            } => {
                let location_name = bundle
                    .locations
                    .iter()
                    .find(|location| location.id == *location_id)
                    .map(|location| location.name.clone())
                    .unwrap_or_else(|| location_id.clone());
                cues.push(MediaCue {
                    label: format!("Noise attractor: {}", location_name),
                    detail: format!(
                        "{} spawned threat(s) turn toward the latest noisy location.",
                        actor_ids.len()
                    ),
                });
            }
            GameEvent::SightAttractorAcquired {
                origin_id,
                subject_id,
                location_id,
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    cues.push(MediaCue {
                        label: format!("Sight chase: {}", enemy.name),
                        detail: format!(
                            "Sighted '{}' at '{}' and started tracking by vision.",
                            subject_id, location_id
                        ),
                    });
                }
            }
            GameEvent::SightAttractorMissed {
                origin_id,
                subject_id,
                location_id,
                detect_chance_percent,
                roll_percent,
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    cues.push(MediaCue {
                        label: format!("Sight miss: {}", enemy.name),
                        detail: format!(
                            "Had a sightline to '{}' at '{}', but rolled {} against {}%.",
                            subject_id, location_id, roll_percent, detect_chance_percent
                        ),
                    });
                }
            }
            GameEvent::SightAttractorLost {
                origin_id,
                subject_id,
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    cues.push(MediaCue {
                        label: format!("Sight lost: {}", enemy.name),
                        detail: format!("Lost visual track of '{}'.", subject_id),
                    });
                }
            }
            GameEvent::ActorMoved {
                origin_id,
                from_location_id,
                to_location_id,
                target_location_id,
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    cues.push(MediaCue {
                        label: format!("Spawned threat moves: {}", enemy.name),
                        detail: format!(
                            "Moved from '{}' to '{}' while tracking '{}'.",
                            from_location_id, to_location_id, target_location_id
                        ),
                    });
                }
            }
            GameEvent::ActorWaited {
                origin_id,
                location_id,
                reason,
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    cues.push(MediaCue {
                        label: format!("Spawned threat waits: {}", enemy.name),
                        detail: format!("Held at '{}': {}.", location_id, reason),
                    });
                }
            }
            GameEvent::ActorAttackedHazard {
                origin_id,
                hazard_kind,
                location_id,
                break_chance_percent,
                roll_percent,
                broken,
            } => {
                if let Some(enemy) = find_enemy(bundle, origin_id) {
                    cues.push(MediaCue {
                        label: format!("Spawned threat attacks hazard: {}", enemy.name),
                        detail: format!(
                            "Attacked {:?} at '{}'. Break chance {}%, roll {}, broken: {}.",
                            hazard_kind, location_id, break_chance_percent, roll_percent, broken
                        ),
                    });
                }
            }
            GameEvent::HelpShown
            | GameEvent::AttackWhiff { .. }
            | GameEvent::StatusEffectExpired { .. }
            | GameEvent::HealthRestored { .. }
            | GameEvent::NarratorStatusAdmitted { .. }
            | GameEvent::StatusEffectApplied { .. }
            | GameEvent::ConnectionsRevealed { .. }
            | GameEvent::AbilityCooldownSet { .. }
            | GameEvent::ProgressAwarded { .. }
            | GameEvent::SkillLevelGained { .. }
            | GameEvent::MaxHpIncreased { .. }
            | GameEvent::AbilityGranted { .. }
            | GameEvent::AbilityGrantExpired { .. } => {}
            GameEvent::NarratorNpcAdmitted { .. } => {}
            GameEvent::NarratorEnemyAdmitted { .. } => {}
            GameEvent::NarratorBossAdmitted { .. } => {}
            GameEvent::NarratorTemporaryExitAdmitted { .. } => {}
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
    current_bundle: Option<&DatapackBundle>,
    current_run: Option<&RunState>,
    recent_events: &[GameEvent],
) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(run) = current_run {
        keys.push(format!("location:{}", run.current_location_id));
        if run.active_objective.completed {
            keys.push("run:won".to_owned());
            if let Some(bundle) = current_bundle
                && let Some(location) = bundle
                    .locations
                    .iter()
                    .find(|location| location.id == run.current_location_id)
                && location.epilogue_hook.is_some()
            {
                keys.push(format!("epilogue_hook:{}", location.id));
            }
        } else if !run.controlled_actor_is_alive() {
            keys.push("run:incapacitated".to_owned());
        } else {
            keys.push(format!("objective:{}", run.active_objective.id));
        }
    }

    if let Some(event) = recent_events.last() {
        match event {
            GameEvent::SocialEventApplied { .. } => {}
            GameEvent::Moved { to_location_id, .. } => {
                keys.push(format!("event:moved:{}", to_location_id))
            }
            GameEvent::LocationLooked { location_id, .. } => {
                keys.push(format!("event:look:{}", location_id))
            }
            GameEvent::Inspected { target, .. } => keys.push(format!("event:inspect:{}", target)),
            GameEvent::ItemTaken { item_id, .. } => {
                keys.push(format!("event:item_taken:{item_id}"))
            }
            GameEvent::ItemTransferred { item_id, .. } => {
                keys.push(format!("event:item_transferred:{item_id}"))
            }
            GameEvent::ItemEquipped { item_id, .. } => {
                keys.push(format!("event:item_equipped:{}", item_id))
            }
            GameEvent::ItemUsed { item_id, .. } => {
                keys.push(format!("event:item_used:{}", item_id))
            }
            GameEvent::ItemConsumed { template_id, .. } => {
                keys.push(format!("event:item_consumed:{}", template_id))
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
                    EncounterKind::Npc => "npc",
                    EncounterKind::Actor => "actor",
                };
                keys.push(format!("event:attack:{}:{}", kind, target_id));
                if *defeated {
                    keys.push(format!("event:defeated:{}:{}", kind, target_id));
                }
            }
            GameEvent::DamageTaken { target_id, .. } => {
                keys.push(format!("event:actor_damaged:{target_id}"));
                // Keep existing datapacks working while allowing actor-specific reaction pools.
                if target_id == "player" {
                    keys.push("event:player_damaged".to_owned());
                }
            }
            GameEvent::ActorSpawnedFromNoise {
                origin_id,
                template_id,
                location_id,
            } => {
                keys.push(format!("event:noise_spawn:{}", origin_id));
                keys.push(format!("event:noise_spawn_template:{}", template_id));
                keys.push(format!("event:noise_spawn_location:{}", location_id));
            }
            GameEvent::NoiseAttractorShifted {
                location_id,
                actor_ids,
            } => {
                keys.push(format!("event:noise_attractor:{}", location_id));
                for actor_id in actor_ids {
                    keys.push(format!("event:noise_attractor_actor:{}", actor_id));
                }
            }
            GameEvent::SightAttractorAcquired {
                origin_id,
                subject_id,
                location_id,
            } => {
                keys.push(format!("event:sight_attractor:{}", location_id));
                keys.push(format!("event:sight_chaser:{}", origin_id));
                keys.push(format!("event:sight_subject:{}", subject_id));
            }
            GameEvent::SightAttractorMissed {
                origin_id,
                subject_id,
                location_id,
                ..
            } => {
                keys.push(format!("event:sight_missed:{}", location_id));
                keys.push(format!("event:sight_miss_chaser:{}", origin_id));
                keys.push(format!("event:sight_miss_subject:{}", subject_id));
            }
            GameEvent::SightAttractorLost {
                origin_id,
                subject_id,
            } => {
                keys.push(format!("event:sight_lost:{}", origin_id));
                keys.push(format!("event:sight_lost_subject:{}", subject_id));
            }
            GameEvent::ActorMoved {
                origin_id,
                from_location_id,
                to_location_id,
                target_location_id,
            } => {
                keys.push(format!("event:actor_moved:{}", origin_id));
                keys.push(format!("event:actor_from:{}", from_location_id));
                keys.push(format!("event:actor_to:{}", to_location_id));
                keys.push(format!("event:actor_target:{}", target_location_id));
            }
            GameEvent::ActorWaited {
                origin_id,
                location_id,
                ..
            } => {
                keys.push(format!("event:actor_waited:{}", origin_id));
                keys.push(format!("event:actor_wait_location:{}", location_id));
            }
            GameEvent::ActorAttackedHazard {
                origin_id,
                hazard_kind,
                location_id,
                broken,
                ..
            } => {
                keys.push(format!("event:actor_hazard_attack:{}", origin_id));
                keys.push(format!("event:actor_hazard:{:?}", hazard_kind));
                keys.push(format!("event:actor_hazard_location:{}", location_id));
                if *broken {
                    keys.push(format!("event:actor_hazard_broken:{}", location_id));
                }
            }
            GameEvent::AbilityUsed {
                origin_id,
                template_id,
                target_id,
                effect,
                ..
            } => {
                keys.push("event:ability_used".to_owned());
                keys.push(format!("event:ability:{template_id}"));
                keys.push(format!("event:ability_origin:{origin_id}"));
                keys.push(format!("event:ability_target:{target_id}"));
                keys.push(format!("event:ability_effect:{effect}"));
            }
            GameEvent::MovementBlocked { .. } => keys.push("event:boundary_blocked".to_owned()),
            GameEvent::LocationUnlocked {
                location_id,
                item_id,
                ..
            } => {
                keys.push(format!("event:location_unlocked:{}", location_id));
                keys.push(format!("event:item_unlock:{}", item_id));
            }
            GameEvent::LocationBarricaded {
                location_id,
                item_id,
                ..
            } => {
                keys.push(format!("event:location_barricaded:{}", location_id));
                keys.push(format!("event:item_barricade:{}", item_id));
            }
            GameEvent::ObjectiveCompleted { objective_id } => {
                keys.push(format!("event:objective_complete:{}", objective_id))
            }
            GameEvent::RunWon => keys.push("event:run_won".to_owned()),
            GameEvent::RunLost { actor_id } => {
                keys.push(format!("event:run_lost:{actor_id}"));
                keys.push("event:run_lost".to_owned());
            }
            GameEvent::Waited { location_id, .. } => {
                keys.push(format!("event:wait:{}", location_id))
            }
            GameEvent::ActionRejected { .. } => keys.push("event:action_rejected".to_owned()),
            GameEvent::SelectedPackFailedValidation { folder_name, .. } => {
                keys.push("event:selected_pack_failed_validation".to_owned());
                keys.push(format!(
                    "event:selected_pack_failed_validation:{}",
                    folder_name
                ));
            }
            GameEvent::HelpShown
            | GameEvent::AttackWhiff { .. }
            | GameEvent::NarratorItemAdmitted { .. }
            | GameEvent::StatusEffectExpired { .. }
            | GameEvent::HealthRestored { .. }
            | GameEvent::NarratorStatusAdmitted { .. }
            | GameEvent::StatusEffectApplied { .. }
            | GameEvent::ConnectionsRevealed { .. }
            | GameEvent::AbilityCooldownSet { .. }
            | GameEvent::ProgressAwarded { .. }
            | GameEvent::SkillLevelGained { .. }
            | GameEvent::MaxHpIncreased { .. }
            | GameEvent::AbilityGranted { .. }
            | GameEvent::AbilityGrantExpired { .. } => {}
            GameEvent::NarratorNpcAdmitted { .. } => {}
            GameEvent::NarratorEnemyAdmitted { .. } => {}
            GameEvent::NarratorBossAdmitted { .. } => {}
            GameEvent::NarratorTemporaryExitAdmitted { .. } => {}
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

    for enemy_id in run
        .enemies_alive
        .iter()
        .filter(|enemy_id| enemy_id.starts_with("noise_spawn_"))
    {
        if let Some(enemy) = find_enemy(bundle, enemy_id) {
            lines.push(format!(
                "Enemy: {} [alive] | spawned instance {}",
                enemy.name, enemy_id
            ));
        }
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

#[allow(clippy::too_many_arguments)]
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

fn player_library_media_selection(
    bundle: &DatapackBundle,
    run: &RunState,
    item: &crate::game::InventoryEntry,
) -> MediaSelection {
    let stable_key = if item.instance_id.is_empty() {
        item.id.as_str()
    } else {
        item.instance_id.as_str()
    };
    player_library_entity_media_selection(
        PlayerLibraryMediaContext {
            bundle,
            current_location: current_location_for_run(bundle, run),
        },
        "item",
        &item.name,
        &item.description,
        &item.tags,
        PlayerLibraryMediaIdentity {
            stable_key,
            datapack_id: &run.datapack_id,
            template_id: &item.template_id,
        },
        "item",
    )
}

fn runtime_actor_media_selection(
    bundle: &DatapackBundle,
    run: &RunState,
    origin_id: &str,
) -> Option<MediaSelection> {
    let actor = run.runtime_actor(origin_id)?;
    let display_role = match actor.presentation_kind.as_str() {
        "NPC" => "npc",
        "Actor item" => "item",
        _ => "actor",
    };
    Some(player_library_entity_media_selection(
        PlayerLibraryMediaContext {
            bundle,
            current_location: current_location_for_run(bundle, run),
        },
        &actor.presentation_kind,
        &actor.name,
        &actor.description,
        &actor.tags,
        PlayerLibraryMediaIdentity {
            stable_key: &actor.entity_id,
            datapack_id: &run.datapack_id,
            template_id: &actor.template_id,
        },
        display_role,
    ))
}

struct PlayerLibraryMediaIdentity<'a> {
    stable_key: &'a str,
    datapack_id: &'a str,
    template_id: &'a str,
}

struct PlayerLibraryMediaContext<'a> {
    bundle: &'a DatapackBundle,
    current_location: Option<&'a LocationTemplate>,
}

fn player_library_entity_media_selection(
    context: PlayerLibraryMediaContext<'_>,
    entity_label: &str,
    name: &str,
    description: &str,
    tags: &[String],
    identity: PlayerLibraryMediaIdentity<'_>,
    display_role: &str,
) -> MediaSelection {
    let pool = load_player_media_pool().unwrap_or_default();
    let exact = load_player_templates_for_datapack(identity.datapack_id)
        .map(|templates| player_template_media_references(&templates, identity.template_id))
        .unwrap_or_default();
    let location_fallback = resolve_media_stack(
        context.bundle,
        context.current_location,
        None,
        ResolutionMode::LocationFirst,
    );
    let image_asset = player_library_media_asset(
        exact.image.as_ref(),
        &pool,
        tags,
        PlayerMediaKind::Image,
        identity.stable_key,
    )
    .or_else(|| {
        datapack_category_pool_asset(
            context.bundle,
            display_role,
            PlayerMediaKind::Image,
            identity.stable_key,
        )
    })
    .or(location_fallback.image_asset.clone());
    let motion_asset = player_library_media_asset(
        exact.motion.as_ref(),
        &pool,
        tags,
        PlayerMediaKind::Motion,
        identity.stable_key,
    )
    .or_else(|| {
        datapack_category_pool_asset(
            context.bundle,
            display_role,
            PlayerMediaKind::Motion,
            identity.stable_key,
        )
    })
    .or(location_fallback.motion_asset.clone());
    let audio_asset = player_library_media_asset(
        exact.audio.as_ref(),
        &pool,
        tags,
        PlayerMediaKind::Audio,
        identity.stable_key,
    )
    .or_else(|| {
        datapack_category_pool_asset(
            context.bundle,
            display_role,
            PlayerMediaKind::Audio,
            identity.stable_key,
        )
    })
    .or(location_fallback.audio_asset.clone());

    MediaSelection {
        label: format!("Player-library {entity_label}: {name}"),
        detail: description.to_owned(),
        narrator_brief: Some(format!(
            "Player-library tags: {}{}",
            tags.join(", "),
            if exact.image.is_some() || exact.motion.is_some() || exact.audio.is_some() {
                " | exact template media preferred"
            } else {
                ""
            }
        )),
        image_asset,
        motion_asset,
        audio_asset,
        display_role: Some(format!("player-library {display_role}")),
        used_datapack_fallback: location_fallback.used_datapack_fallback,
        used_engine_fallback: location_fallback.used_engine_fallback,
    }
}

fn current_location_for_run<'a>(
    bundle: &'a DatapackBundle,
    run: &RunState,
) -> Option<&'a LocationTemplate> {
    bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id)
}

/// A proposal-created entity has no pack template of its own. Once exact player-template media
/// and the tagged player pool are exhausted, the host may still present it through an authored
/// category pool. This is presentation-only: it neither changes the admitted entity nor claims
/// that the selected asset was authored for that exact runtime instance.
fn datapack_category_pool_asset(
    bundle: &DatapackBundle,
    category: &str,
    kind: PlayerMediaKind,
    stable_key: &str,
) -> Option<MediaAssetStatus> {
    let references = match category {
        "item" => bundle
            .items
            .iter()
            .map(|template| &template.media)
            .collect::<Vec<_>>(),
        "npc" => bundle
            .npcs
            .iter()
            .map(|template| &template.media)
            .collect::<Vec<_>>(),
        "enemy" => bundle
            .enemies
            .iter()
            .map(|template| &template.media)
            .collect::<Vec<_>>(),
        "boss" => bundle
            .bosses
            .iter()
            .map(|template| &template.media)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let wanted_fields: &[&str] = match kind {
        PlayerMediaKind::Image => &["image"],
        PlayerMediaKind::Motion => &["gif", "video"],
        PlayerMediaKind::Audio => &["audio"],
    };
    let mut candidates = references
        .into_iter()
        .flat_map(|media| resolve_media_assets(bundle, media))
        .filter(|asset| asset.present && wanted_fields.contains(&asset.field_name.as_str()))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.resolved_path.cmp(&right.resolved_path));
    let index = stable_key.bytes().fold(0usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as usize)
    }) % candidates.len().max(1);
    candidates.get(index).map(|asset| MediaAssetStatus {
        relative_path: format!("datapack {category} pool: {}", asset.relative_path),
        resolved_path: asset.resolved_path.clone(),
        present: true,
    })
}

fn player_template_media_references(
    templates: &[crate::collection::PlayerTemplate],
    template_id: &str,
) -> crate::collection::PlayerTemplateMediaReferences {
    templates
        .iter()
        .find(|template| template.id == template_id && template.enabled)
        .map(|template| template.media_references.clone())
        .unwrap_or_default()
}

fn player_library_media_asset(
    exact_path: Option<&String>,
    pool: &crate::media_pool::PlayerMediaPool,
    tags: &[String],
    kind: PlayerMediaKind,
    stable_key: &str,
) -> Option<MediaAssetStatus> {
    exact_path
        .map(|path| MediaAssetStatus {
            relative_path: format!("player template: {path}"),
            present: std::path::Path::new(path).is_file(),
            resolved_path: path.clone(),
        })
        .or_else(|| {
            select_pool_asset(pool, tags, kind, stable_key).map(|asset| MediaAssetStatus {
                relative_path: format!("player library: {}", asset.path),
                present: std::path::Path::new(&asset.path).is_file(),
                resolved_path: asset.path,
            })
        })
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
    let pool_tags = focus
        .map(|(id, media)| {
            let mut tags = media.pool_tags.clone();
            tags.push(id.to_owned());
            if let Some(role) = &media.display_role {
                tags.push(role.clone());
            }
            tags
        })
        .unwrap_or_default();
    let pool_key = focus.map_or("location", |(id, _)| id);
    let pool_asset = |kind| pool_media_asset(&pool_tags, kind, pool_key);

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
            .or_else(|| pool_asset(PlayerMediaKind::Image))
            .or(location_image)
            .or_else(|| resolve_datapack_image_fallback(bundle))
            .or_else(engine_image_fallback),
    };
    let motion_asset = match mode {
        ResolutionMode::LocationFirst => override_motion.or(location_motion),
        ResolutionMode::FocusFirst => override_motion
            .or(focus_motion)
            .or_else(|| pool_asset(PlayerMediaKind::Motion))
            .or(location_motion),
    };
    let audio_asset = match mode {
        ResolutionMode::LocationFirst => override_audio.or(location_audio),
        ResolutionMode::FocusFirst => override_audio
            .or(focus_audio)
            .or_else(|| pool_asset(PlayerMediaKind::Audio))
            .or(location_audio),
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

fn pool_media_asset(
    tags: &[String],
    kind: PlayerMediaKind,
    stable_key: &str,
) -> Option<MediaAssetStatus> {
    let pool = load_player_media_pool().ok()?;
    let asset = select_pool_asset(&pool, tags, kind, stable_key)?;
    Some(MediaAssetStatus {
        relative_path: asset.path.clone(),
        resolved_path: asset.path.clone(),
        present: std::path::Path::new(&asset.path).is_file(),
    })
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

    let enemy_ids = run
        .location_enemies
        .get(&location.id)
        .unwrap_or(&location.enemies);
    for enemy_id in enemy_ids {
        if run.enemies_alive.contains(enemy_id)
            && let Some(enemy) = find_enemy(bundle, enemy_id)
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
                pool_tags: Vec::new(),
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
                pool_tags: Vec::new(),
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
            .find(|it| it.id == enemy_template_id(target_id))
            .map(|it| (it.name.clone(), it.description.clone()))
            .unwrap_or_else(|| (target_id.to_owned(), "Unknown enemy target.".to_owned())),
        EncounterKind::Boss => bundle
            .bosses
            .iter()
            .find(|it| it.id == target_id)
            .map(|it| (it.name.clone(), it.description.clone()))
            .unwrap_or_else(|| (target_id.to_owned(), "Unknown boss target.".to_owned())),
        EncounterKind::Npc => bundle
            .npcs
            .iter()
            .find(|it| it.id == target_id)
            .map(|it| (it.name.clone(), it.description.clone()))
            .unwrap_or_else(|| (target_id.to_owned(), "Unknown NPC target.".to_owned())),
        EncounterKind::Actor => (
            target_id.to_owned(),
            "A runtime actor-capable item or entity.".to_owned(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::collection::{
        PlayerTemplate, PlayerTemplateAdmission, PlayerTemplateKind, PlayerTemplateMechanics,
        PlayerTemplateMediaReferences,
    };
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{EncounterKind, GameEvent};
    use crate::game::generation::generate_new_run;
    use crate::game::state::{AbilityGrant, AbilityGrantSource, ItemOwner};
    use crate::game::{
        ActorDisposition, ActorState, EncounterBucket, EncounterInstance, InstanceLifecycle,
        ItemBucket, ItemOrigin, NpcInstance,
    };
    use crate::media_pool::{PlayerMediaAsset, PlayerMediaKind, PlayerMediaPool};

    use super::{
        build_media_panel_state, build_media_panel_state_with_focus, player_library_media_asset,
        player_library_media_selection, player_template_media_references,
    };

    #[test]
    fn player_library_selection_keeps_an_explicit_provenance_label() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let item = state.inventory.first().expect("starter item");
        let selection = player_library_media_selection(&bundle, &state, item);
        assert!(selection.label.starts_with("Player-library item:"));
        assert_eq!(
            selection.display_role.as_deref(),
            Some("player-library item")
        );
    }

    #[test]
    fn exact_player_template_media_beats_a_matching_pool_asset() {
        let pool = PlayerMediaPool {
            assets: vec![PlayerMediaAsset {
                path: "pool_image.png".to_owned(),
                tags: vec!["guide".to_owned()],
                kind: PlayerMediaKind::Image,
                enabled: true,
            }],
        };
        let tags = vec!["guide".to_owned()];
        let exact = "exact_image.png".to_owned();

        let selected = player_library_media_asset(
            Some(&exact),
            &pool,
            &tags,
            PlayerMediaKind::Image,
            "fixture",
        )
        .expect("exact reference resolves into a presentation selection");

        assert_eq!(selected.resolved_path, "exact_image.png");
        assert!(selected.relative_path.starts_with("player template:"));
    }

    #[test]
    fn disabled_player_template_exact_media_is_not_eligible_for_resolution() {
        let template = PlayerTemplate {
            id: "disabled.fixture".to_owned(),
            kind: PlayerTemplateKind::Item,
            name: "Disabled fixture".to_owned(),
            description: "A media fixture.".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Item {
                damage: 0,
                condition: None,
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: "fixture".to_owned(),
            enabled: false,
            provenance: "test".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences {
                image: Some("disabled.png".to_owned()),
                motion: None,
                audio: None,
            },
        };

        assert!(
            player_template_media_references(&[template], "disabled.fixture")
                .image
                .is_none()
        );
    }

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
            origin_id: "player".to_owned(),
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
    fn media_panel_keeps_dynamic_item_focus_after_a_transfer_to_the_world() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        let item = crate::game::InventoryEntry {
            id: "player_template.dropped_fixture".to_owned(),
            instance_id: "runtime_dropped_fixture".to_owned(),
            template_id: "player_template.dropped_fixture".to_owned(),
            bucket: ItemBucket::World,
            location_id: Some(location_id.clone()),
            lifecycle: InstanceLifecycle::Active,
            name: "Dropped fixture".to_owned(),
            description: "A dynamic item used to verify transfer presentation.".to_owned(),
            damage: 0,
            origin: ItemOrigin::NarratorApproved,
            condition: None,
            tags: vec!["fixture".to_owned()],
            actor: None,
            disposition: ActorDisposition::Neutral,
            inventory: Vec::new(),
            equipped_item_id: None,
        };
        state
            .world_item_instances
            .insert(item.instance_id.clone(), item);

        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::ItemTransferred {
                item_id: "player_template.dropped_fixture".to_owned(),
                origin: ItemOwner::Player,
                destination: ItemOwner::Location(location_id),
            }],
        );

        assert!(media.title.contains("Player-library item: Dropped fixture"));
        assert_eq!(
            media.selected_display_role.as_deref(),
            Some("player-library item")
        );
        assert!(
            media
                .active_cues
                .iter()
                .any(|cue| cue.label == "Player-library transfer: Dropped fixture")
        );
    }

    #[test]
    fn media_panel_focuses_a_narrator_admitted_runtime_npc() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "runtime_npc_1".to_owned(),
                template_id: "runtime_npc_1".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "narrator_approved".to_owned(),
                name: "Runtime Guide".to_owned(),
                description: "A dynamically admitted media fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["guide".to_owned(), "trusting".to_owned()],
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.ability_grants.push(AbilityGrant {
            instance_id: "runtime_guide_grant".to_owned(),
            template_id: "grave_bolt".to_owned(),
            owner: ItemOwner::Entity("runtime_npc_1".to_owned()),
            remaining_uses: Some(2),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Innate,
        });

        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::NarratorNpcAdmitted {
                instance_id: "runtime_npc_1".to_owned(),
            }],
        );

        assert!(media.title.contains("Player-library NPC: Runtime Guide"));
        assert_eq!(
            media.selected_display_role.as_deref(),
            Some("player-library npc")
        );
        assert!(media.observed_entity_rows.iter().any(|row| {
            row.contains("[NPC] Runtime Guide")
                && row.contains("disposition friendly")
                && row.contains("tags: guide, trusting")
                && row.contains("personality trusting")
                && row.contains("inventory: empty")
                && row.contains(
                    "abilities: Grave Bolt (2; ready; unavailable: missing required tag 'can_necromancy')",
                )
        }));
    }

    #[test]
    fn dynamic_enemy_uses_the_authored_enemy_media_pool_when_no_player_media_is_available() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.enemy_instances.insert(
            "runtime_enemy_1".to_owned(),
            EncounterInstance {
                instance_id: "runtime_enemy_1".to_owned(),
                template_id: "scratch_enemy_1".to_owned(),
                location_id: state.current_location_id.clone(),
                bucket: EncounterBucket::Live,
                lifecycle: InstanceLifecycle::Active,
                current_hp: 8,
                max_hp: 8,
                provenance: "narrator_approved".to_owned(),
                name: "Runtime Threat".to_owned(),
                description: "A test-only dynamic threat.".to_owned(),
                damage: 2,
                traversal_cost: 1,
                attack_cost: 1,
                tags: vec!["unfamiliar".to_owned()],
                capabilities: vec!["health".to_owned(), "combat".to_owned()],
            },
        );
        state.enemies_alive.insert("runtime_enemy_1".to_owned());
        state
            .location_enemies
            .entry(state.current_location_id.clone())
            .or_default()
            .push("runtime_enemy_1".to_owned());

        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::NarratorEnemyAdmitted {
                instance_id: "runtime_enemy_1".to_owned(),
            }],
        );

        assert!(media.title.contains("Player-library Enemy: Runtime Threat"));
        assert!(
            media
                .selected_image
                .as_ref()
                .is_some_and(|asset| asset.relative_path.starts_with("datapack enemy pool:")),
            "a dynamic enemy should select an authored enemy-pool image before falling back"
        );

        let focused = build_media_panel_state_with_focus(
            Some(&bundle),
            Some(&state),
            &[],
            Some("runtime_enemy_1"),
        );
        assert_eq!(
            focused.focused_entity_id.as_deref(),
            Some("runtime_enemy_1")
        );
        assert!(focused.observed_entities.iter().any(|entity| {
            entity.id == "runtime_enemy_1"
                && entity.label.starts_with("[Enemy]")
                && entity.detail.contains("A test-only dynamic threat.")
                && entity.detail.contains("tags: unfamiliar")
        }));
    }

    #[test]
    fn explicit_focus_overrides_recent_npc_activity_for_an_authored_enemy() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = state.location_enemies[&state.current_location_id][0].clone();
        state.enemies_alive.insert(enemy_id.clone());
        let focused_instance_id = format!("authored_enemy_{enemy_id}");

        let media = build_media_panel_state_with_focus(
            Some(&bundle),
            Some(&state),
            &[GameEvent::NarratorNpcAdmitted {
                instance_id: "unrelated_recent_npc".to_owned(),
            }],
            Some(&focused_instance_id),
        );

        assert!(
            media
                .observed_entities
                .iter()
                .any(|entity| entity.id == focused_instance_id),
            "expected {} in {:?}",
            focused_instance_id,
            media
                .observed_entities
                .iter()
                .map(|entity| entity.id.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            media
                .observed_entities
                .iter()
                .filter(|entity| entity.id == focused_instance_id)
                .count(),
            1,
            "an authored encounter must have exactly one observable focus row"
        );
        assert_eq!(
            media.focused_entity_id.as_deref(),
            Some(focused_instance_id.as_str())
        );
        assert!(media.title.starts_with("Threat focus:"));
        assert_eq!(media.selected_display_role.as_deref(), Some("enemy"));
    }

    #[test]
    fn media_panel_keeps_runtime_npc_focus_after_an_autonomous_actor_event() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let from_location_id = state.current_location_id.clone();
        let to_location_id = "kitchen".to_owned();
        state.location_npcs.insert(
            to_location_id.clone(),
            vec![NpcInstance {
                instance_id: "runtime_npc_actor".to_owned(),
                template_id: "runtime_npc_actor".to_owned(),
                location_id: to_location_id.clone(),
                lifecycle: InstanceLifecycle::Active,
                provenance: "narrator_approved".to_owned(),
                name: "Runtime Actor".to_owned(),
                description: "A dynamically admitted actor media fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["actor".to_owned()],
                disposition: ActorDisposition::Friendly,
                actor: ActorState::default(),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );

        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::ActorMoved {
                origin_id: "runtime_npc_actor".to_owned(),
                from_location_id,
                to_location_id: to_location_id.clone(),
                target_location_id: to_location_id,
            }],
        );

        assert!(media.title.contains("Player-library NPC: Runtime Actor"));
        assert_eq!(
            media.selected_display_role.as_deref(),
            Some("player-library npc")
        );
    }

    #[test]
    fn media_panel_keeps_runtime_actor_item_focus_after_an_autonomous_actor_event() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        let item = crate::game::InventoryEntry {
            id: "runtime_actor_item".to_owned(),
            instance_id: "runtime_actor_item".to_owned(),
            template_id: "runtime_actor_item".to_owned(),
            bucket: ItemBucket::World,
            location_id: Some(location_id.clone()),
            lifecycle: InstanceLifecycle::Active,
            name: "Runtime Pack".to_owned(),
            description: "A dynamic actor-item media fixture.".to_owned(),
            damage: 0,
            origin: ItemOrigin::NarratorApproved,
            condition: None,
            tags: vec!["actor".to_owned(), "pack".to_owned()],
            actor: Some(ActorState::default()),
            disposition: ActorDisposition::Friendly,
            inventory: Vec::new(),
            equipped_item_id: None,
        };
        state
            .location_items
            .entry(location_id.clone())
            .or_default()
            .push(item.instance_id.clone());
        state
            .world_item_instances
            .insert(item.instance_id.clone(), item);

        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::ActorMoved {
                origin_id: "runtime_actor_item".to_owned(),
                from_location_id: location_id.clone(),
                to_location_id: location_id.clone(),
                target_location_id: location_id,
            }],
        );

        assert!(
            media
                .title
                .contains("Player-library Actor item: Runtime Pack")
        );
        assert_eq!(
            media.selected_display_role.as_deref(),
            Some("player-library item")
        );
    }

    #[test]
    fn media_panel_focuses_a_runtime_npc_after_reducer_confirmed_combat() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "runtime_npc_target".to_owned(),
                template_id: "runtime_npc_target".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "narrator_approved".to_owned(),
                name: "Runtime Target".to_owned(),
                description: "A combat media fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["target".to_owned()],
                disposition: ActorDisposition::Hostile,
                actor: ActorState::default(),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );

        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::AttackResolved {
                origin_id: "player".to_owned(),
                target_id: "runtime_npc_target".to_owned(),
                target_kind: EncounterKind::Npc,
                damage: 1,
                defeated: false,
            }],
        );

        assert!(media.title.contains("Player-library NPC: Runtime Target"));
        assert_eq!(
            media.selected_display_role.as_deref(),
            Some("player-library npc")
        );
    }

    #[test]
    fn media_panel_shifts_focus_to_boss_combat_event() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        let events = vec![GameEvent::AttackResolved {
            origin_id: "player".to_owned(),
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

    #[test]
    fn ability_event_exposes_presentation_pool_keys_without_owning_mechanics() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let state = generate_new_run(&bundle).state;
        let media = build_media_panel_state(
            Some(&bundle),
            Some(&state),
            &[GameEvent::AbilityUsed {
                origin_id: "player".to_owned(),
                grant_id: "starter_ability_grave_bolt".to_owned(),
                template_id: "grave_bolt".to_owned(),
                target_id: "shambler_front_gate".to_owned(),
                effect: "damage".to_owned(),
                remaining_uses: None,
            }],
        );

        assert!(
            media
                .active_cues
                .iter()
                .any(|cue| cue.label == "Ability damage: grave_bolt")
        );
        for key in [
            "event:ability_used",
            "event:ability:grave_bolt",
            "event:ability_origin:player",
            "event:ability_target:shambler_front_gate",
            "event:ability_effect:damage",
        ] {
            assert!(
                media
                    .future_hook_keys
                    .iter()
                    .any(|candidate| candidate == key),
                "missing media hook key {key}"
            );
        }
    }

    #[test]
    fn media_panel_surfaces_epilogue_hook_key_after_win() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.completed = true;
        state.current_location_id = "garage".to_owned();

        let media = build_media_panel_state(Some(&bundle), Some(&state), &[]);

        assert!(
            media
                .future_hook_keys
                .iter()
                .any(|key| key == "epilogue_hook:garage")
        );
        assert!(media.future_hook_keys.iter().any(|key| key == "run:won"));
    }
}
