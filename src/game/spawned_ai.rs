use std::collections::{HashMap, HashSet, VecDeque};

use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, GameEvent, MovementHazardKind};
use super::capabilities::{self, EngineCapability};
use super::effects::{adjusted_timing_cost, consume_timing_effects, evaluate_timing_effects};
use super::movement::handle_move;
use super::queries::{find_boss, find_enemy, find_location};
use super::reducer::{apply_actor_status_effects, expire_scene_duration_effects_after_movement};
use super::state::{ActionTimingKind, EffectTrigger, InstanceLifecycle, RunState};
pub(super) fn apply_autonomous_actor_turns(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
) {
    let spawned_this_turn = outcome
        .events
        .iter()
        .filter_map(|event| {
            if let GameEvent::ActorSpawnedFromNoise { origin_id, .. } = event {
                Some(origin_id.clone())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();
    let mut spawned_ids = autonomous_actor_ids(state);
    spawned_ids.retain(|actor_id| spawned_actor_can_move(state, bundle, actor_id));
    spawned_ids.sort();

    for enemy_id in spawned_ids {
        if !state.actor_is_ready(&enemy_id) {
            continue;
        }
        if spawned_this_turn.contains(&enemy_id) {
            continue;
        }
        let Some(current_location_id) = spawned_enemy_location(state, &enemy_id) else {
            continue;
        };
        let sighted_this_turn = refresh_spawned_enemy_sight_target(
            state,
            bundle,
            outcome,
            &enemy_id,
            &current_location_id,
        );
        let attractor = spawned_enemy_active_attractor(state, &enemy_id);
        let target_location_id = attractor.location_id.clone();
        if current_location_id == target_location_id
            && !matches!(attractor.kind, SpawnedEnemyAttractorKind::Sight)
        {
            state.actor_searching.insert(enemy_id.clone());
        }
        let is_searching = state.actor_searching.contains(&enemy_id);

        let mut step = spawned_enemy_next_step(
            state,
            bundle,
            &enemy_id,
            &current_location_id,
            &target_location_id,
            is_searching,
        );
        if matches!(attractor.kind, SpawnedEnemyAttractorKind::Sight)
            && !is_searching
            && matches!(step, SpawnedEnemyStep::Move { .. })
            && sight_chase_should_delay(
                bundle,
                state,
                &enemy_id,
                &current_location_id,
                &target_location_id,
            )
        {
            state.actor_sight_delays.insert(enemy_id.clone(), 1);
            step = SpawnedEnemyStep::Wait(format!(
                "chasing {} by sight takes an extra moment",
                state
                    .actor_sight_subjects
                    .get(&enemy_id)
                    .map(String::as_str)
                    .unwrap_or("the attractor")
            ));
        } else if sighted_this_turn {
            state.actor_sight_delays.remove(&enemy_id);
        }

        match step {
            SpawnedEnemyStep::Move {
                to_location_id,
                step_target_location_id,
            } => {
                let context_tags = find_location(bundle, &current_location_id)
                    .map(|location| location.tags.as_slice())
                    .unwrap_or_default();
                let timing = evaluate_timing_effects(
                    state,
                    &enemy_id,
                    &[ActionTimingKind::Movement],
                    context_tags,
                );
                if timing.blocked_by_instance_id.is_some() {
                    continue;
                }
                let mut movement = handle_actor_move(
                    state,
                    bundle,
                    &enemy_id,
                    &to_location_id,
                    step_target_location_id
                        .as_deref()
                        .or(Some(&target_location_id)),
                );
                if movement.events.iter().any(|event| {
                    matches!(event, GameEvent::ActorMoved { origin_id, .. } if origin_id == &enemy_id)
                }) {
                    apply_actor_status_effects(
                        state,
                        bundle,
                        &enemy_id,
                        EffectTrigger::Movement,
                        &[],
                        &mut movement,
                    );
                    expire_scene_duration_effects_after_movement(
                        state,
                        &enemy_id,
                        &current_location_id,
                        &mut movement,
                    );
                    let cost = adjusted_timing_cost(
                        state.actor_traversal_cost(&enemy_id),
                        timing.cost_delta,
                    );
                    state.commit_actor_action_time(&enemy_id, cost);
                    outcome.events.extend(
                        consume_timing_effects(state, &timing.consume_on_success)
                            .into_iter()
                            .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
                    );
                }
                outcome.events.extend(movement.events);
                outcome.lines.extend(movement.lines);
            }
            SpawnedEnemyStep::Wait(reason) => {
                let context_tags = find_location(bundle, &current_location_id)
                    .map(|location| location.tags.as_slice())
                    .unwrap_or_default();
                let timing = evaluate_timing_effects(
                    state,
                    &enemy_id,
                    &[ActionTimingKind::Wait],
                    context_tags,
                );
                if timing.blocked_by_instance_id.is_some() {
                    continue;
                }
                state.commit_actor_action_time(
                    &enemy_id,
                    adjusted_timing_cost(1, timing.cost_delta),
                );
                outcome.events.push(GameEvent::ActorWaited {
                    origin_id: enemy_id.clone(),
                    location_id: current_location_id.clone(),
                    reason: reason.clone(),
                });
                outcome.lines.push(format!(
                    "{} waits at {}: {}.",
                    actor_display_name(state, bundle, &enemy_id),
                    location_display_name(bundle, &current_location_id),
                    reason
                ));
                apply_actor_status_effects(
                    state,
                    bundle,
                    &enemy_id,
                    EffectTrigger::Wait,
                    &[],
                    outcome,
                );
                outcome.events.extend(
                    consume_timing_effects(state, &timing.consume_on_success)
                        .into_iter()
                        .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
                );
            }
            SpawnedEnemyStep::AttackHazard(hazard) => {
                if !spawned_actor_has_combat_contract(state, bundle, &enemy_id) {
                    // A tracking/movement contract may lead an actor to a barricade, but it
                    // does not grant an implicit combat branch.  Leave the hazard untouched
                    // rather than emitting a decorative attack event.
                    outcome.lines.push(format!(
                        "{} cannot attack the {}: its combat capability is unavailable.",
                        actor_display_name(state, bundle, &enemy_id),
                        match hazard.kind {
                            MovementHazardKind::Barricade => "barricade",
                            MovementHazardKind::LockedGate => "locked gate",
                        }
                    ));
                    continue;
                }
                let context_tags = find_location(bundle, &current_location_id)
                    .map(|location| location.tags.as_slice())
                    .unwrap_or_default();
                let timing = evaluate_timing_effects(
                    state,
                    &enemy_id,
                    &[ActionTimingKind::Combat],
                    context_tags,
                );
                if timing.blocked_by_instance_id.is_some() {
                    continue;
                }
                let break_chance_percent = spawned_hazard_break_chance_percent(bundle);
                let roll_percent =
                    deterministic_hazard_break_roll(state, &enemy_id, &hazard.location_id);
                let broken = roll_percent < break_chance_percent;
                if broken {
                    break_movement_hazard(state, &hazard);
                }
                state.commit_actor_action_time(
                    &enemy_id,
                    adjusted_timing_cost(state.actor_attack_cost(&enemy_id), timing.cost_delta),
                );
                outcome.events.push(GameEvent::ActorAttackedHazard {
                    origin_id: enemy_id.clone(),
                    hazard_kind: hazard.kind.clone(),
                    location_id: hazard.location_id.clone(),
                    break_chance_percent,
                    roll_percent,
                    broken,
                });
                outcome.lines.push(spawned_enemy_hazard_attack_line(
                    state, bundle, &enemy_id, &hazard, broken,
                ));
                // A hazard strike is still a reducer-confirmed combat action.  It does not get
                // a noun-specific effect lane merely because its impact is a gate or barricade:
                // actor-owned combat statuses must trigger and expire exactly as they do for a
                // hostile actor attack.
                apply_actor_status_effects(
                    state,
                    bundle,
                    &enemy_id,
                    EffectTrigger::Combat,
                    &[],
                    outcome,
                );
                outcome.events.extend(
                    consume_timing_effects(state, &timing.consume_on_success)
                        .into_iter()
                        .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
                );
            }
        }
    }
}

/// Shared reducer implementation for an actor movement request. The existing noise/sight loop and
/// direct player instructions both enter here, so NPCs and actor-capable world items never need a
/// noun-specific movement implementation.
pub(crate) fn handle_actor_move(
    state: &mut RunState,
    bundle: &DatapackBundle,
    actor_id: &str,
    destination: &str,
    target_location_id: Option<&str>,
) -> ActionOutcome {
    if actor_id == state.player_entity_id && actor_id == "player" {
        // Player movement remains on the compatibility reducer for fog, objective, and save
        // semantics, but arrives through the same actor dispatcher as every other origin.
        return handle_move(state, bundle, destination);
    }
    let Some(from_location_id) = spawned_enemy_location(state, actor_id) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Actor is not active in the world.".to_owned(),
            }],
            lines: vec!["That actor cannot move from its current state.".to_owned()],
        };
    };
    if !spawned_actor_can_move(state, bundle, actor_id) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Actor lacks the movement capability contract.".to_owned(),
            }],
            lines: vec![format!(
                "{} cannot move.",
                actor_display_name(state, bundle, actor_id)
            )],
        };
    }
    if !legal_spawned_enemy_exits(state, bundle, &from_location_id)
        .iter()
        .any(|exit_id| exit_id == destination)
    {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Destination is not a legal adjacent route for that actor.".to_owned(),
            }],
            lines: vec![format!(
                "{} cannot reach that destination from here.",
                actor_display_name(state, bundle, actor_id)
            )],
        };
    }

    move_spawned_enemy(state, actor_id, &from_location_id, destination);
    if actor_id == state.player_entity_id {
        state.current_location_id = destination.to_owned();
        state.known_locations.insert(destination.to_owned());
        state.visited_locations.insert(destination.to_owned());
        if let Some(instance) = state.location_instances.get_mut(destination) {
            instance.known = true;
            instance.visited = true;
        }
    }
    let target_location_id = target_location_id.unwrap_or(destination).to_owned();
    ActionOutcome {
        events: vec![GameEvent::ActorMoved {
            origin_id: actor_id.to_owned(),
            from_location_id: from_location_id.clone(),
            to_location_id: destination.to_owned(),
            target_location_id,
        }],
        lines: vec![format!(
            "{} shifts from {} toward {}.",
            actor_display_name(state, bundle, actor_id),
            location_display_name(bundle, &from_location_id),
            location_display_name(bundle, destination)
        )],
    }
}

/// Resolves a named actor movement instruction into a bounded shared action. It identifies only
/// existing actors and authored locations; validation and mutation still happen in
/// `handle_actor_move` through the reducer.
pub fn resolve_actor_move_intent(
    state: &RunState,
    bundle: &DatapackBundle,
    input: &str,
) -> Option<Result<(String, String), String>> {
    let normalized = input.to_ascii_lowercase();
    let movement_word = ["go", "move", "walk", "head"].iter().any(|word| {
        normalized
            .split(|character: char| !character.is_alphabetic())
            .any(|token| token == *word)
    });
    if !movement_word {
        return None;
    }

    let location_id = state.current_location_id.as_str();
    let actor = state
        .runtime_actor_named_at(location_id, &normalized)
        .map(|actor| (actor.entity_id, actor.name))?;
    if !spawned_actor_can_move(state, bundle, &actor.0) {
        return Some(Err(format!(
            "{} cannot move: its movement capability is unavailable.",
            actor.1
        )));
    }
    let destination = bundle
        .locations
        .iter()
        .find(|location| {
            normalized.contains(&location.id.to_ascii_lowercase())
                || normalized.contains(&location.name.to_ascii_lowercase())
        })
        .map(|location| location.id.clone());
    Some(
        destination
            .ok_or_else(|| "Name an authored destination for that actor.".to_owned())
            .map(|destination| (actor.0, destination)),
    )
}

pub(super) enum SpawnedEnemyStep {
    Move {
        to_location_id: String,
        step_target_location_id: Option<String>,
    },
    Wait(String),
    AttackHazard(MovementHazard),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SpawnedEnemyAttractorKind {
    Sight,
    Noise,
    SearchFallback,
}

pub(super) struct SpawnedEnemyAttractor {
    pub(super) kind: SpawnedEnemyAttractorKind,
    pub(super) location_id: String,
}

pub(super) struct MovementHazard {
    pub(super) kind: MovementHazardKind,
    pub(super) location_id: String,
}

#[derive(Clone)]
struct SightAttractor {
    subject_id: String,
    location_id: String,
}

pub(super) fn spawned_enemy_active_attractor(
    state: &RunState,
    enemy_id: &str,
) -> SpawnedEnemyAttractor {
    if let Some(location_id) = state.actor_sight_targets.get(enemy_id) {
        return SpawnedEnemyAttractor {
            kind: SpawnedEnemyAttractorKind::Sight,
            location_id: location_id.clone(),
        };
    }

    if let Some(location_id) = state.actor_noise_targets.get(enemy_id) {
        return SpawnedEnemyAttractor {
            kind: SpawnedEnemyAttractorKind::Noise,
            location_id: location_id.clone(),
        };
    }

    SpawnedEnemyAttractor {
        kind: SpawnedEnemyAttractorKind::SearchFallback,
        location_id: state.current_location_id.clone(),
    }
}

pub(super) fn refresh_spawned_enemy_sight_target(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
    enemy_id: &str,
    current_location_id: &str,
) -> bool {
    if !spawned_enemy_can_see(state, bundle, enemy_id) {
        clear_spawned_enemy_sight_to_search(state, bundle, outcome, enemy_id);
        return false;
    }

    if let Some(attractor) =
        visible_sight_attractor_for_enemy(state, bundle, enemy_id, current_location_id)
    {
        let roll_percent =
            deterministic_sight_acquire_roll(state, enemy_id, current_location_id, &attractor);
        let detect_chance_percent = sight_acquire_chance_percent(bundle);
        if roll_percent >= detect_chance_percent {
            outcome.events.push(GameEvent::SightAttractorMissed {
                origin_id: enemy_id.to_owned(),
                subject_id: attractor.subject_id.clone(),
                location_id: attractor.location_id.clone(),
                detect_chance_percent,
                roll_percent,
            });
            outcome.lines.push(format!(
                "{} {}.",
                actor_display_name(state, bundle, enemy_id),
                sight_miss_flavor_line(state, enemy_id, &attractor.subject_id)
            ));
            clear_spawned_enemy_sight_to_search(state, bundle, outcome, enemy_id);
            return false;
        }

        let target_changed = state
            .actor_sight_targets
            .get(enemy_id)
            .is_none_or(|location_id| location_id != &attractor.location_id)
            || state
                .actor_sight_subjects
                .get(enemy_id)
                .is_none_or(|subject_id| subject_id != &attractor.subject_id);
        state
            .actor_sight_targets
            .insert(enemy_id.to_owned(), attractor.location_id.clone());
        state
            .actor_sight_subjects
            .insert(enemy_id.to_owned(), attractor.subject_id.clone());
        state.actor_searching.remove(enemy_id);
        if target_changed {
            outcome.events.push(GameEvent::SightAttractorAcquired {
                origin_id: enemy_id.to_owned(),
                subject_id: attractor.subject_id.clone(),
                location_id: attractor.location_id.clone(),
            });
            outcome.lines.push(format!(
                "{} catches sight of {} at {}.",
                actor_display_name(state, bundle, enemy_id),
                subject_display_name(state, bundle, &attractor.subject_id),
                location_display_name(bundle, &attractor.location_id)
            ));
        }
        return true;
    }

    clear_spawned_enemy_sight_to_search(state, bundle, outcome, enemy_id);

    false
}

fn clear_spawned_enemy_sight_to_search(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
    enemy_id: &str,
) -> bool {
    if state.actor_sight_targets.contains_key(enemy_id)
        || state.actor_sight_delays.contains_key(enemy_id)
    {
        state.actor_sight_delays.remove(enemy_id);
        let subject_id = state
            .actor_sight_subjects
            .remove(enemy_id)
            .unwrap_or_else(|| "the last seen attractor".to_owned());
        state.actor_sight_targets.remove(enemy_id);
        state.actor_searching.insert(enemy_id.to_owned());
        outcome.events.push(GameEvent::SightAttractorLost {
            origin_id: enemy_id.to_owned(),
            subject_id: subject_id.clone(),
        });
        outcome.lines.push(format!(
            "{} {}.",
            actor_display_name(state, bundle, enemy_id),
            sight_lost_flavor_line(state, enemy_id, &subject_id)
        ));
        return true;
    }
    false
}

fn visible_sight_attractor_for_enemy(
    state: &RunState,
    bundle: &DatapackBundle,
    enemy_id: &str,
    current_location_id: &str,
) -> Option<SightAttractor> {
    if state.controlled_actor_is_alive()
        && spawned_enemy_can_see_location(
            state,
            bundle,
            current_location_id,
            &state.current_location_id,
        )
    {
        return Some(SightAttractor {
            subject_id: state.player_entity_id.clone(),
            location_id: state.current_location_id.clone(),
        });
    }

    if state.actor_noise_targets.contains_key(enemy_id) && !state.actor_searching.contains(enemy_id)
    {
        return None;
    }

    let mut subjects = Vec::new();
    for (location_id, enemy_ids) in &state.location_enemies {
        if !spawned_enemy_can_see_location(state, bundle, current_location_id, location_id) {
            continue;
        }
        for subject_id in enemy_ids {
            if subject_id != enemy_id && state.enemies_alive.contains(subject_id) {
                subjects.push(SightAttractor {
                    subject_id: subject_id.clone(),
                    location_id: location_id.clone(),
                });
            }
        }
    }
    for (location_id, boss_ids) in &state.location_bosses {
        if !spawned_enemy_can_see_location(state, bundle, current_location_id, location_id) {
            continue;
        }
        for subject_id in boss_ids {
            if state.bosses_alive.contains(subject_id) {
                subjects.push(SightAttractor {
                    subject_id: subject_id.clone(),
                    location_id: location_id.clone(),
                });
            }
        }
    }
    for (location_id, npcs) in &state.location_npcs {
        if !spawned_enemy_can_see_location(state, bundle, current_location_id, location_id) {
            continue;
        }
        for npc in npcs {
            if npc.lifecycle == InstanceLifecycle::Active
                && is_noise_spawned_actor(&npc.instance_id)
                && npc.instance_id != enemy_id
            {
                subjects.push(SightAttractor {
                    subject_id: npc.instance_id.clone(),
                    location_id: location_id.clone(),
                });
            }
        }
    }
    subjects.sort_by(|left, right| {
        left.location_id
            .cmp(&right.location_id)
            .then_with(|| left.subject_id.cmp(&right.subject_id))
    });
    subjects.into_iter().next()
}

fn spawned_enemy_can_see_location(
    state: &RunState,
    bundle: &DatapackBundle,
    from_location_id: &str,
    to_location_id: &str,
) -> bool {
    if from_location_id == to_location_id {
        return true;
    }
    legal_spawned_enemy_exits(state, bundle, from_location_id)
        .iter()
        .any(|exit_id| exit_id == to_location_id)
}

pub(super) fn spawned_enemy_next_step(
    state: &RunState,
    bundle: &DatapackBundle,
    enemy_id: &str,
    current_location_id: &str,
    target_location_id: &str,
    is_searching: bool,
) -> SpawnedEnemyStep {
    if is_searching {
        return spawned_enemy_search_step(state, bundle, enemy_id, current_location_id);
    }
    if current_location_id == target_location_id {
        return SpawnedEnemyStep::Wait("already at the attractor".to_owned());
    }
    if state.barricaded_locations.contains(current_location_id) {
        return SpawnedEnemyStep::AttackHazard(MovementHazard {
            kind: MovementHazardKind::Barricade,
            location_id: current_location_id.to_owned(),
        });
    }

    if spawned_enemy_uses_path_to_attractor(bundle) {
        if let Some(hazard) = next_blocking_hazard_toward_target(
            state,
            bundle,
            current_location_id,
            target_location_id,
        ) {
            return SpawnedEnemyStep::AttackHazard(hazard);
        }
        if let Some(next_step) = shortest_legal_spawned_enemy_step(
            state,
            bundle,
            current_location_id,
            target_location_id,
        ) {
            return SpawnedEnemyStep::Move {
                to_location_id: next_step,
                step_target_location_id: None,
            };
        }
        return SpawnedEnemyStep::Wait("no legal route reaches the noise source".to_owned());
    }

    let legal_exits = legal_spawned_enemy_exits(state, bundle, current_location_id);
    if legal_exits.is_empty() {
        return SpawnedEnemyStep::Wait("no legal exit is open".to_owned());
    }
    let index = super::noise::deterministic_noise_index(
        state,
        legal_exits.len(),
        enemy_id.bytes().fold(97usize, |accumulator, byte| {
            accumulator.wrapping_mul(31).wrapping_add(byte as usize)
        }),
    );
    SpawnedEnemyStep::Move {
        to_location_id: legal_exits[index].clone(),
        step_target_location_id: None,
    }
}

fn spawned_enemy_search_step(
    state: &RunState,
    bundle: &DatapackBundle,
    enemy_id: &str,
    current_location_id: &str,
) -> SpawnedEnemyStep {
    match super::noise::deterministic_noise_index(
        state,
        3,
        enemy_search_salt(enemy_id, current_location_id, 211),
    ) {
        0 => SpawnedEnemyStep::Wait(search_wait_flavor_line(
            state,
            enemy_id,
            current_location_id,
        )),
        1 => spawned_enemy_random_search_step(state, bundle, enemy_id, current_location_id),
        _ => spawned_enemy_return_step(state, bundle, current_location_id, enemy_id),
    }
}

fn spawned_enemy_random_search_step(
    state: &RunState,
    bundle: &DatapackBundle,
    enemy_id: &str,
    current_location_id: &str,
) -> SpawnedEnemyStep {
    let legal_exits = legal_spawned_enemy_exits(state, bundle, current_location_id);
    if legal_exits.is_empty() {
        return SpawnedEnemyStep::Wait(search_blocked_flavor_line(
            state,
            enemy_id,
            current_location_id,
        ));
    }
    let index = super::noise::deterministic_noise_index(
        state,
        legal_exits.len(),
        enemy_search_salt(enemy_id, current_location_id, 307),
    );
    SpawnedEnemyStep::Move {
        to_location_id: legal_exits[index].clone(),
        step_target_location_id: None,
    }
}

fn spawned_enemy_return_step(
    state: &RunState,
    bundle: &DatapackBundle,
    current_location_id: &str,
    enemy_id: &str,
) -> SpawnedEnemyStep {
    let Some(origin_location_id) = state.actor_origins.get(enemy_id) else {
        return SpawnedEnemyStep::Wait("searching without a known return point".to_owned());
    };
    if current_location_id == origin_location_id {
        return SpawnedEnemyStep::Wait(returned_origin_flavor_line(
            state,
            enemy_id,
            current_location_id,
        ));
    }
    if spawned_enemy_uses_path_to_attractor(bundle) {
        if let Some(hazard) = next_blocking_hazard_toward_target(
            state,
            bundle,
            current_location_id,
            origin_location_id,
        ) {
            return SpawnedEnemyStep::AttackHazard(hazard);
        }
        if let Some(next_step) = shortest_legal_spawned_enemy_step(
            state,
            bundle,
            current_location_id,
            origin_location_id,
        ) {
            return SpawnedEnemyStep::Move {
                to_location_id: next_step,
                step_target_location_id: Some(origin_location_id.clone()),
            };
        }
        return SpawnedEnemyStep::Wait(return_blocked_flavor_line(
            state,
            enemy_id,
            current_location_id,
        ));
    }

    spawned_enemy_random_search_step(state, bundle, enemy_id, current_location_id)
}

fn next_blocking_hazard_toward_target(
    state: &RunState,
    bundle: &DatapackBundle,
    current_location_id: &str,
    target_location_id: &str,
) -> Option<MovementHazard> {
    let next_step =
        shortest_map_step_ignoring_hazards(bundle, current_location_id, target_location_id)?;
    movement_hazard_for_location(state, &next_step)
}

fn movement_hazard_for_location(state: &RunState, location_id: &str) -> Option<MovementHazard> {
    if state.barricaded_locations.contains(location_id) {
        Some(MovementHazard {
            kind: MovementHazardKind::Barricade,
            location_id: location_id.to_owned(),
        })
    } else if state.locked_locations.contains(location_id) {
        Some(MovementHazard {
            kind: MovementHazardKind::LockedGate,
            location_id: location_id.to_owned(),
        })
    } else {
        None
    }
}

fn shortest_map_step_ignoring_hazards(
    bundle: &DatapackBundle,
    current_location_id: &str,
    target_location_id: &str,
) -> Option<String> {
    find_location(bundle, target_location_id)?;

    let mut queue = VecDeque::from([current_location_id.to_owned()]);
    let mut previous = HashMap::from([(current_location_id.to_owned(), None::<String>)]);

    while let Some(location_id) = queue.pop_front() {
        if location_id == target_location_id {
            break;
        }

        let Some(location) = find_location(bundle, &location_id) else {
            continue;
        };

        for exit_id in &location.connections {
            if find_location(bundle, exit_id).is_none() || previous.contains_key(exit_id) {
                continue;
            }
            previous.insert(exit_id.clone(), Some(location_id.clone()));
            queue.push_back(exit_id.clone());
        }
    }

    previous.get(target_location_id)?;
    let mut step = target_location_id.to_owned();
    while let Some(Some(parent)) = previous.get(&step) {
        if parent == current_location_id {
            return Some(step);
        }
        step = parent.clone();
    }
    None
}

pub(super) fn deterministic_hazard_break_roll(
    state: &RunState,
    enemy_id: &str,
    hazard_location_id: &str,
) -> u8 {
    let seed = format!(
        "{}:{}:{}:{}:{}",
        enemy_id, hazard_location_id, state.noise_spawn_count, state.turn_index, state.noise_level
    );
    (seed.bytes().fold(0usize, |accumulator, byte| {
        accumulator.wrapping_mul(33).wrapping_add(byte as usize)
    }) % 100) as u8
}

pub(super) fn spawned_hazard_break_chance_percent(bundle: &DatapackBundle) -> u8 {
    bundle.rules.spawned_hazard_break_chance_percent.min(100)
}

fn spawned_enemy_uses_path_to_attractor(bundle: &DatapackBundle) -> bool {
    bundle.rules.spawned_enemy_movement_policy == "path_to_attractor"
}

fn sight_acquire_chance_percent(bundle: &DatapackBundle) -> u8 {
    bundle.rules.sight_acquire_chance_percent.min(100)
}

fn sight_chase_delay_chance_percent(bundle: &DatapackBundle) -> u8 {
    bundle.rules.sight_chase_delay_chance_percent.min(100)
}

pub(super) fn sight_chase_should_delay(
    bundle: &DatapackBundle,
    state: &RunState,
    enemy_id: &str,
    current_location_id: &str,
    target_location_id: &str,
) -> bool {
    deterministic_sight_chase_roll(state, enemy_id, current_location_id, target_location_id)
        < sight_chase_delay_chance_percent(bundle)
}

fn deterministic_sight_acquire_roll(
    state: &RunState,
    enemy_id: &str,
    current_location_id: &str,
    attractor: &SightAttractor,
) -> u8 {
    let seed = format!(
        "{}:{}:{}:{}:{}:{}",
        enemy_id,
        current_location_id,
        attractor.subject_id,
        attractor.location_id,
        state.turn_index,
        state.noise_spawn_count
    );
    (seed.bytes().fold(0usize, |accumulator, byte| {
        accumulator.wrapping_mul(37).wrapping_add(byte as usize)
    }) % 100) as u8
}

fn deterministic_sight_chase_roll(
    state: &RunState,
    enemy_id: &str,
    current_location_id: &str,
    target_location_id: &str,
) -> u8 {
    let seed = format!(
        "{}:{}:{}:{}:{}:{}",
        enemy_id,
        current_location_id,
        target_location_id,
        state.current_location_id,
        state.turn_index,
        state.noise_spawn_count
    );
    (seed.bytes().fold(0usize, |accumulator, byte| {
        accumulator.wrapping_mul(31).wrapping_add(byte as usize)
    }) % 100) as u8
}

pub(super) fn break_movement_hazard(state: &mut RunState, hazard: &MovementHazard) {
    match hazard.kind {
        MovementHazardKind::Barricade => {
            state.barricaded_locations.remove(&hazard.location_id);
        }
        MovementHazardKind::LockedGate => {
            state.locked_locations.remove(&hazard.location_id);
            if let Some(instance) = state.location_instances.get_mut(&hazard.location_id) {
                instance.locked = false;
            }
            state
                .broken_locked_locations
                .insert(hazard.location_id.clone());
        }
    }
}

pub(super) fn spawned_enemy_hazard_attack_line(
    state: &RunState,
    bundle: &DatapackBundle,
    enemy_id: &str,
    hazard: &MovementHazard,
    broken: bool,
) -> String {
    let enemy_name = actor_display_name(state, bundle, enemy_id);
    let location_name = location_display_name(bundle, &hazard.location_id);
    match (&hazard.kind, broken) {
        (MovementHazardKind::Barricade, true) => {
            format!("{enemy_name} tears through the barricade at {location_name}.")
        }
        (MovementHazardKind::Barricade, false) => {
            format!("{enemy_name} hammers the barricade at {location_name}, but it holds.")
        }
        (MovementHazardKind::LockedGate, true) => {
            format!("{enemy_name} breaks the locked gate at {location_name}.")
        }
        (MovementHazardKind::LockedGate, false) => {
            format!(
                "{enemy_name} throws itself at the locked gate at {location_name}, but it holds."
            )
        }
    }
}

fn shortest_legal_spawned_enemy_step(
    state: &RunState,
    bundle: &DatapackBundle,
    current_location_id: &str,
    target_location_id: &str,
) -> Option<String> {
    find_location(bundle, target_location_id)?;

    let mut queue = VecDeque::from([current_location_id.to_owned()]);
    let mut previous = HashMap::from([(current_location_id.to_owned(), None::<String>)]);

    while let Some(location_id) = queue.pop_front() {
        if location_id == target_location_id {
            break;
        }

        for exit_id in legal_spawned_enemy_exits(state, bundle, &location_id) {
            if previous.contains_key(&exit_id) {
                continue;
            }
            previous.insert(exit_id.clone(), Some(location_id.clone()));
            queue.push_back(exit_id);
        }
    }

    previous.get(target_location_id)?;
    let mut step = target_location_id.to_owned();
    while let Some(Some(parent)) = previous.get(&step) {
        if parent == current_location_id {
            return Some(step);
        }
        step = parent.clone();
    }
    None
}

fn legal_spawned_enemy_exits(
    state: &RunState,
    bundle: &DatapackBundle,
    location_id: &str,
) -> Vec<String> {
    if state.barricaded_locations.contains(location_id) {
        return Vec::new();
    }

    let Some(location) = find_location(bundle, location_id) else {
        return Vec::new();
    };

    location
        .connections
        .iter()
        .filter(|exit_id| find_location(bundle, exit_id).is_some())
        .filter(|exit_id| !state.locked_locations.contains(*exit_id))
        .filter(|exit_id| !state.barricaded_locations.contains(*exit_id))
        .cloned()
        .collect()
}

pub(super) fn move_spawned_enemy(
    state: &mut RunState,
    enemy_id: &str,
    from_location_id: &str,
    to_location_id: &str,
) {
    if let Some(entries) = state.location_enemies.get_mut(from_location_id) {
        entries.retain(|entry| entry != enemy_id);
    }
    if state.bosses_alive.contains(enemy_id) {
        if let Some(entries) = state.location_bosses.get_mut(from_location_id) {
            entries.retain(|entry| entry != enemy_id);
        }
        let destination = state
            .location_bosses
            .entry(to_location_id.to_owned())
            .or_default();
        if !destination.iter().any(|entry| entry == enemy_id) {
            destination.push(enemy_id.to_owned());
        }
        if let Some(instance) = state.boss_instances.get_mut(enemy_id) {
            instance.location_id = to_location_id.to_owned();
        }
        return;
    }
    if let Some((from_bucket, npc_index)) =
        state.location_npcs.iter().find_map(|(location_id, npcs)| {
            (location_id == from_location_id)
                .then(|| {
                    npcs.iter()
                        .position(|npc| npc.instance_id == enemy_id)
                        .map(|index| (location_id.clone(), index))
                })
                .flatten()
        })
    {
        let mut npc = state
            .location_npcs
            .get_mut(&from_bucket)
            .and_then(|npcs| (npc_index < npcs.len()).then(|| npcs.remove(npc_index)))
            .expect("spawned NPC location must remain stable during movement");
        npc.location_id = to_location_id.to_owned();
        state
            .location_npcs
            .entry(to_location_id.to_owned())
            .or_default()
            .push(npc);
        return;
    }
    if let Some(item) = state.world_item_instances.get_mut(enemy_id) {
        if let Some(entries) = state.location_items.get_mut(from_location_id) {
            entries.retain(|entry| entry != enemy_id);
        }
        item.location_id = Some(to_location_id.to_owned());
        state
            .location_items
            .entry(to_location_id.to_owned())
            .or_default()
            .push(enemy_id.to_owned());
        return;
    }
    let destination = state
        .location_enemies
        .entry(to_location_id.to_owned())
        .or_default();
    if !destination.iter().any(|entry| entry == enemy_id) {
        destination.push(enemy_id.to_owned());
    }
}

pub(super) fn spawned_enemy_location(state: &RunState, enemy_id: &str) -> Option<String> {
    state
        .location_enemies
        .iter()
        .find(|(_, enemy_ids)| enemy_ids.iter().any(|entry| entry == enemy_id))
        .map(|(location_id, _)| location_id.clone())
        .or_else(|| {
            state
                .location_bosses
                .iter()
                .find(|(_, actor_ids)| actor_ids.iter().any(|entry| entry == enemy_id))
                .map(|(location_id, _)| location_id.clone())
        })
        .or_else(|| {
            state
                .world_item_instances
                .get(enemy_id)
                .filter(|item| item.lifecycle == InstanceLifecycle::Active)
                .and_then(|item| item.location_id.clone())
        })
        .or_else(|| {
            state.location_npcs.iter().find_map(|(location_id, npcs)| {
                npcs.iter()
                    .any(|npc| {
                        npc.instance_id == enemy_id && npc.lifecycle == InstanceLifecycle::Active
                    })
                    .then(|| location_id.clone())
            })
        })
}

pub(super) fn is_noise_spawned_enemy(enemy_id: &str) -> bool {
    enemy_id.starts_with("noise_spawn_")
}

pub(super) fn is_noise_spawned_actor(actor_id: &str) -> bool {
    is_noise_spawned_enemy(actor_id)
}

/// Actors enter the autonomous tracking loop only when they have a host-owned tracking policy.
/// Legacy enemy/boss instances retain their historical noise-spawn behavior; runtime NPCs and
/// world items opt in explicitly through noise or sight tracking rather than merely having legs.
pub(super) fn autonomous_actor_ids(state: &RunState) -> Vec<String> {
    let mut ids = state
        .enemies_alive
        .iter()
        .chain(state.bosses_alive.iter())
        .filter(|id| is_noise_spawned_actor(id))
        .cloned()
        .collect::<Vec<_>>();
    ids.extend(state.location_npcs.values().flat_map(|npcs| {
        npcs.iter()
            .filter(|npc| npc.lifecycle == InstanceLifecycle::Active)
            .filter(|npc| {
                is_noise_spawned_actor(&npc.instance_id)
                    || npc.actor.has_capability(EngineCapability::NoiseTrack)
                    || npc.actor.has_capability(EngineCapability::SightTrack)
            })
            .map(|npc| npc.instance_id.clone())
    }));
    ids.extend(
        state
            .world_item_instances
            .values()
            .filter(|item| {
                item.lifecycle == InstanceLifecycle::Active
                    && item.actor.as_ref().is_some_and(|actor| {
                        is_noise_spawned_actor(&item.instance_id)
                            || actor.has_capability(EngineCapability::NoiseTrack)
                            || actor.has_capability(EngineCapability::SightTrack)
                    })
            })
            .map(|item| item.instance_id.clone()),
    );
    ids.sort();
    ids.dedup();
    ids
}

pub(super) fn spawned_actor_can_hear(
    state: &RunState,
    bundle: &DatapackBundle,
    actor_id: &str,
) -> bool {
    // A persisted encounter is the authoritative mutable actor. Its template is only a
    // compatibility fallback for old noise-spawn ids that have not yet been materialized.
    if let Some(actor) = state.runtime_actor(actor_id) {
        return actor.actor.has_hearing_contract()
            && actor.actor.has_capability(EngineCapability::NoiseTrack);
    }
    if let Some(enemy) = find_enemy(bundle, actor_id) {
        return enemy.can_hear
            && (enemy.capabilities.is_empty()
                || (capabilities::contains(&enemy.capabilities, EngineCapability::Hearing)
                    && capabilities::contains(&enemy.capabilities, EngineCapability::NoiseTrack)));
    }
    if let Some(boss) = find_boss(bundle, actor_id) {
        return boss.can_hear
            && capabilities::contains(&boss.capabilities, EngineCapability::Hearing)
            && capabilities::contains(&boss.capabilities, EngineCapability::NoiseTrack);
    }
    false
}

fn spawned_enemy_can_see(state: &RunState, bundle: &DatapackBundle, enemy_id: &str) -> bool {
    if let Some(actor) = state.runtime_actor(enemy_id) {
        return actor.actor.has_sight_contract()
            && actor.actor.has_capability(EngineCapability::SightTrack);
    }
    if let Some(enemy) = find_enemy(bundle, enemy_id) {
        return enemy.can_see
            && (enemy.capabilities.is_empty()
                || (capabilities::contains(&enemy.capabilities, EngineCapability::Sight)
                    && capabilities::contains(&enemy.capabilities, EngineCapability::SightTrack)));
    }
    if let Some(boss) = find_boss(bundle, enemy_id) {
        return boss.can_see
            && capabilities::contains(&boss.capabilities, EngineCapability::Sight)
            && capabilities::contains(&boss.capabilities, EngineCapability::SightTrack);
    }
    false
}

fn spawned_actor_can_move(state: &RunState, bundle: &DatapackBundle, actor_id: &str) -> bool {
    if let Some(actor) = state.runtime_actor(actor_id) {
        return actor.actor.has_movement_contract();
    }
    if let Some(enemy) = find_enemy(bundle, actor_id) {
        return enemy.capabilities.is_empty()
            || capabilities::contains(&enemy.capabilities, EngineCapability::Movement);
    }
    if let Some(boss) = find_boss(bundle, actor_id) {
        return capabilities::contains(&boss.capabilities, EngineCapability::Movement);
    }
    false
}

/// New spawned encounters carry their own persisted capability envelope.  A pre-migration save
/// can still contain a legacy `noise_spawn_*` id without an instance record, so fall back to its
/// immutable template only for that compatibility shape.
fn spawned_actor_has_combat_contract(
    state: &RunState,
    bundle: &DatapackBundle,
    actor_id: &str,
) -> bool {
    if let Some(actor) = state.runtime_actor(actor_id) {
        return actor.actor.is_combatant();
    }
    if let Some(enemy) = find_enemy(bundle, actor_id) {
        return enemy.capabilities.is_empty()
            || (capabilities::contains(&enemy.capabilities, EngineCapability::Health)
                && capabilities::contains(&enemy.capabilities, EngineCapability::Combat));
    }
    if let Some(boss) = find_boss(bundle, actor_id) {
        return boss.capabilities.is_empty()
            || (capabilities::contains(&boss.capabilities, EngineCapability::Health)
                && capabilities::contains(&boss.capabilities, EngineCapability::Combat));
    }
    false
}

fn enemy_search_salt(enemy_id: &str, location_id: &str, base: usize) -> usize {
    format!("{enemy_id}:{location_id}")
        .bytes()
        .fold(base, |accumulator, byte| {
            accumulator.wrapping_mul(31).wrapping_add(byte as usize)
        })
}

fn deterministic_flavor_index(
    state: &RunState,
    enemy_id: &str,
    context_id: &str,
    len: usize,
    salt: usize,
) -> usize {
    super::noise::deterministic_noise_index(
        state,
        len,
        enemy_search_salt(enemy_id, context_id, salt),
    )
}

fn search_wait_flavor_line(state: &RunState, enemy_id: &str, location_id: &str) -> String {
    let variants = [
        "searching the old noise source",
        "searching the stale trail by habit",
        "searching where the noise used to make sense",
    ];
    variants[deterministic_flavor_index(state, enemy_id, location_id, variants.len(), 401)]
        .to_owned()
}

fn search_blocked_flavor_line(state: &RunState, enemy_id: &str, location_id: &str) -> String {
    let variants = [
        "searching, but no legal exit is open",
        "searching the boxed-in space without finding a way through",
        "searching, stalled by every closed route around it",
    ];
    variants[deterministic_flavor_index(state, enemy_id, location_id, variants.len(), 409)]
        .to_owned()
}

fn returned_origin_flavor_line(state: &RunState, enemy_id: &str, location_id: &str) -> String {
    let variants = [
        "back at its spawn point with no fresh noise",
        "back where it started, with nothing new to follow",
        "back at origin, waiting for the next mistake",
    ];
    variants[deterministic_flavor_index(state, enemy_id, location_id, variants.len(), 419)]
        .to_owned()
}

fn return_blocked_flavor_line(state: &RunState, enemy_id: &str, location_id: &str) -> String {
    let variants = [
        "no legal route back to its spawn point",
        "searching for a way home, but the route will not open",
        "trying to return to origin, but the map refuses",
    ];
    variants[deterministic_flavor_index(state, enemy_id, location_id, variants.len(), 421)]
        .to_owned()
}

fn sight_miss_flavor_line(state: &RunState, enemy_id: &str, subject_id: &str) -> String {
    let variants = [
        "has a sightline toward the attractor, but does not catch a clear look",
        "nearly catches the visual attractor, but the moment slips",
        "stares down the sightline and still misses the trail",
    ];
    variants[deterministic_flavor_index(state, enemy_id, subject_id, variants.len(), 431)]
        .to_owned()
}

fn sight_lost_flavor_line(state: &RunState, enemy_id: &str, subject_id: &str) -> String {
    let variants = [
        "loses sight of the attractor and starts searching",
        "loses the visual trail and starts searching",
        "loses contact and falls back into search",
    ];
    variants[deterministic_flavor_index(state, enemy_id, subject_id, variants.len(), 433)]
        .to_owned()
}

fn actor_display_name(state: &RunState, bundle: &DatapackBundle, actor_id: &str) -> String {
    state
        .runtime_actor(actor_id)
        .map(|actor| actor.name)
        .or_else(|| {
            find_enemy(bundle, actor_id)
                .map(|enemy| enemy.name.clone())
                .or_else(|| find_boss(bundle, actor_id).map(|boss| boss.name.clone()))
        })
        .unwrap_or_else(|| actor_id.to_owned())
}

fn subject_display_name(state: &RunState, bundle: &DatapackBundle, subject_id: &str) -> String {
    if subject_id == "player" {
        return "you".to_owned();
    }
    state
        .runtime_actor(subject_id)
        .map(|actor| actor.name)
        .or_else(|| find_enemy(bundle, subject_id).map(|enemy| enemy.name.clone()))
        .or_else(|| find_boss(bundle, subject_id).map(|boss| boss.name.clone()))
        .unwrap_or_else(|| subject_id.to_owned())
}

pub(super) fn location_display_name(bundle: &DatapackBundle, location_id: &str) -> String {
    find_location(bundle, location_id)
        .map(|location| location.name.clone())
        .unwrap_or_else(|| location_id.to_owned())
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::generation::generate_new_run;

    use super::{spawned_actor_can_hear, spawned_actor_can_move, spawned_enemy_can_see};

    #[test]
    fn persisted_encounter_capabilities_override_its_legacy_template_for_ai() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let encounter_id = state
            .enemy_instances
            .keys()
            .next()
            .expect("fixture enemy")
            .clone();
        state
            .enemy_instances
            .get_mut(&encounter_id)
            .expect("fixture encounter")
            .capabilities = vec!["health".to_owned()];

        assert!(!spawned_actor_can_hear(&state, &bundle, &encounter_id));
        assert!(!spawned_enemy_can_see(&state, &bundle, &encounter_id));
        assert!(!spawned_actor_can_move(&state, &bundle, &encounter_id));
    }
}
