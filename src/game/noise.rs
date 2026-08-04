use crate::data::datapacks::{DatapackBundle, EnemyTemplate, LocationTemplate};

use super::actions::{ActionOutcome, GameAction, GameEvent};
use super::spawned_ai::{is_noise_spawned_enemy, spawned_enemy_can_hear};
use super::state::RunState;

const MAX_NOISE_LEVEL: i32 = 3;

pub(super) fn apply_noise_for_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: &GameAction,
    outcome: &mut ActionOutcome,
) {
    match action {
        GameAction::Attack => raise_noise(state, bundle, 1, outcome),
        GameAction::Unlock { .. } => {
            if outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LocationUnlocked { .. }))
            {
                raise_noise(state, bundle, 1, outcome);
            }
        }
        GameAction::Barricade { .. } => {
            if outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LocationBarricaded { .. }))
            {
                raise_noise(state, bundle, 1, outcome);
            }
        }
        _ => lower_noise(state, 1, &mut outcome.lines),
    }
}

fn raise_noise(
    state: &mut RunState,
    bundle: &DatapackBundle,
    amount: i32,
    outcome: &mut ActionOutcome,
) {
    let before = state.noise_level;
    state.noise_level = (state.noise_level + amount).clamp(0, MAX_NOISE_LEVEL);
    if state.noise_level != before {
        outcome.lines.push(format!(
            "Noise rises to {}.",
            noise_label(state.noise_level)
        ));
    }
    retarget_spawned_enemies_to_noise_source(state, bundle, outcome);
    if before < MAX_NOISE_LEVEL && state.noise_level == MAX_NOISE_LEVEL {
        spawn_noise_enemy(state, bundle, outcome);
    }
}

fn lower_noise(state: &mut RunState, amount: i32, lines: &mut Vec<String>) {
    let before = state.noise_level;
    state.noise_level = (state.noise_level - amount).clamp(0, MAX_NOISE_LEVEL);
    if state.noise_level != before {
        lines.push(format!(
            "Noise settles to {}.",
            noise_label(state.noise_level)
        ));
    }
}

fn spawn_noise_enemy(state: &mut RunState, bundle: &DatapackBundle, outcome: &mut ActionOutcome) {
    let Some(enemy) = select_noise_spawn_enemy(state, bundle) else {
        return;
    };
    let Some(location) = select_noise_spawn_location(state, bundle) else {
        return;
    };

    let spawn_number = state.noise_spawn_count + 1;
    let enemy_id = format!("noise_spawn_{}_{}", spawn_number, enemy.id);
    state.noise_spawn_count = spawn_number;
    state.enemy_hp.insert(enemy_id.clone(), enemy.hp);
    state.enemies_alive.insert(enemy_id.clone());
    state
        .spawned_enemy_targets
        .insert(enemy_id.clone(), state.current_location_id.clone());
    state
        .spawned_enemy_origins
        .insert(enemy_id.clone(), location.id.clone());
    state.spawned_enemy_searching.remove(&enemy_id);
    state.spawned_enemy_sight_targets.remove(&enemy_id);
    state.spawned_enemy_sight_subjects.remove(&enemy_id);
    state.spawned_enemy_sight_delays.remove(&enemy_id);
    state
        .location_enemies
        .entry(location.id.clone())
        .or_default()
        .push(enemy_id.clone());

    outcome.events.push(GameEvent::NoiseSpawnedEnemy {
        enemy_id,
        template_id: enemy.id.clone(),
        location_id: location.id.clone(),
    });
    outcome.lines.push(format!(
        "Noise peaks at Swarming. {} is pulled into {}.",
        enemy.name, location.name
    ));
}

fn retarget_spawned_enemies_to_noise_source(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
) {
    let source_location_id = state.current_location_id.clone();
    let mut retargeted_enemy_ids = state
        .enemies_alive
        .iter()
        .filter(|enemy_id| is_noise_spawned_enemy(enemy_id))
        .filter(|enemy_id| spawned_enemy_can_hear(bundle, enemy_id))
        .filter(|enemy_id| {
            state
                .spawned_enemy_targets
                .get(*enemy_id)
                .is_none_or(|target| target != &source_location_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    retargeted_enemy_ids.sort();

    if retargeted_enemy_ids.is_empty() {
        return;
    }

    for enemy_id in &retargeted_enemy_ids {
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), source_location_id.clone());
        state.spawned_enemy_searching.remove(enemy_id);
        state.spawned_enemy_sight_targets.remove(enemy_id);
        state.spawned_enemy_sight_subjects.remove(enemy_id);
        state.spawned_enemy_sight_delays.remove(enemy_id);
    }

    outcome.events.push(GameEvent::NoiseAttractorShifted {
        location_id: source_location_id.clone(),
        enemy_ids: retargeted_enemy_ids,
    });
    outcome.lines.push(format!(
        "The latest noise becomes the attractor. Spawned threats turn toward {}.",
        location_display_name_for_state_only(&source_location_id)
    ));
}

fn location_display_name_for_state_only(location_id: &str) -> String {
    location_id.replace('_', " ")
}

fn select_noise_spawn_enemy(state: &RunState, bundle: &DatapackBundle) -> Option<EnemyTemplate> {
    if bundle.enemies.is_empty() {
        return None;
    }
    let enemies = bundle
        .enemies
        .iter()
        .filter(|enemy| enemy.can_hear)
        .collect::<Vec<_>>();
    if enemies.is_empty() {
        return None;
    }
    let index = deterministic_noise_index(state, enemies.len(), 17);
    enemies.get(index).map(|enemy| (*enemy).clone())
}

fn select_noise_spawn_location(
    state: &RunState,
    bundle: &DatapackBundle,
) -> Option<LocationTemplate> {
    let locations = bundle
        .locations
        .iter()
        .filter(|location| {
            location.tags.iter().any(|tag| tag == "outdoor")
                || location.id.contains("yard")
                || location.id.contains("garden")
        })
        .collect::<Vec<_>>();
    if locations.is_empty() {
        return None;
    }
    let index = deterministic_noise_index(state, locations.len(), 53);
    locations.get(index).map(|location| (*location).clone())
}

pub(super) fn deterministic_noise_index(state: &RunState, len: usize, salt: usize) -> usize {
    let location_score = state
        .current_location_id
        .bytes()
        .fold(0usize, |accumulator, byte| {
            accumulator.wrapping_mul(31).wrapping_add(byte as usize)
        });
    let turn_score = (state.turn_index as usize).wrapping_mul(13);
    let spawn_score = (state.noise_spawn_count as usize).wrapping_mul(37);
    location_score
        .wrapping_add(turn_score)
        .wrapping_add(spawn_score)
        .wrapping_add(salt)
        % len
}

fn noise_label(level: i32) -> &'static str {
    match level {
        0 => "Quiet",
        1 => "Stirred",
        2 => "Loud",
        _ => "Swarming",
    }
}

pub(super) fn exposed_noise_pressure_damage(state: &RunState) -> i32 {
    if state.noise_level >= 2 { 2 } else { 1 }
}
