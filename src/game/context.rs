use crate::data::datapacks::{DatapackBundle, ItemTemplate, LocationTemplate};

use super::derived::boss_wounded_phase_active;
use super::queries::{find_boss, find_enemy};
use super::state::RunState;

pub(super) fn movement_context_lines(
    state: &RunState,
    bundle: &DatapackBundle,
    location: &LocationTemplate,
) -> Vec<String> {
    let mut lines = Vec::new();

    let boss_alive = state
        .location_bosses
        .get(&location.id)
        .into_iter()
        .flatten()
        .any(|boss_id| state.bosses_alive.contains(boss_id));

    if boss_alive {
        lines.extend(location.movement_context_lines.clone());
        if finale_security_partially_secured(state, bundle)
            && let Some(line) = location.movement_context_secured_line.as_deref()
        {
            lines.push(line.to_owned());
        }
    }

    lines
}

fn finale_security_partially_secured(state: &RunState, bundle: &DatapackBundle) -> bool {
    bundle
        .rules
        .finale_secured_location_ids
        .iter()
        .any(|location_id| state.barricaded_locations.contains(location_id))
}

pub(super) fn item_context_lines(state: &RunState, item: &ItemTemplate) -> Vec<String> {
    let mut lines = item.inspect_lines.clone();
    let objective_line = if state.active_objective.required_item_id.as_deref() == Some(&item.id) {
        item.objective_required_line.as_deref()
    } else {
        item.objective_not_required_line.as_deref()
    };
    if let Some(objective_line) = objective_line {
        lines.push(objective_line.to_owned());
    }
    lines
}

pub(super) fn inspect_enemy_state_lines(
    state: &RunState,
    bundle: &DatapackBundle,
    enemy_id: &str,
) -> Vec<String> {
    let alive = state.enemies_alive.contains(enemy_id);
    let remaining_hp = state.enemy_hp.get(enemy_id).copied().unwrap_or(0).max(0);
    let present_here = state
        .location_enemies
        .get(&state.current_location_id)
        .into_iter()
        .flatten()
        .any(|current_id| current_id == enemy_id && alive);

    let mut lines = vec![format!(
        "Threat state: {} | HP remaining: {}",
        if alive { "active" } else { "defeated" },
        remaining_hp
    )];

    if alive {
        lines.push(format!(
            "Present here: {}",
            if present_here { "yes" } else { "no" }
        ));
    }

    if let Some(enemy) = find_enemy(bundle, enemy_id) {
        lines.push(sense_line(enemy.can_hear, enemy.can_see));
        let inspect_line = if alive {
            enemy.inspect_alive_line.as_deref()
        } else {
            enemy.inspect_defeated_line.as_deref()
        };
        if let Some(inspect_line) = inspect_line {
            lines.push(inspect_line.to_owned());
        }
    }

    lines
}

pub(super) fn inspect_boss_state_lines(
    state: &RunState,
    bundle: &DatapackBundle,
    boss_id: &str,
) -> Vec<String> {
    let alive = state.bosses_alive.contains(boss_id);
    let remaining_hp = state.boss_hp.get(boss_id).copied().unwrap_or(0).max(0);
    let present_here = state
        .location_bosses
        .get(&state.current_location_id)
        .into_iter()
        .flatten()
        .any(|current_id| current_id == boss_id && alive);

    let mut lines = vec![format!(
        "Threat state: {} | HP remaining: {}",
        if alive { "active" } else { "defeated" },
        remaining_hp
    )];

    if alive {
        lines.push(format!(
            "Present here: {}",
            if present_here { "yes" } else { "no" }
        ));
    }

    if let Some(boss) = find_boss(bundle, boss_id) {
        lines.push(sense_line(boss.can_hear, boss.can_see));

        if boss_wounded_phase_active(boss, remaining_hp) {
            lines.push(
                boss.wounded_phase_inspect_active
                    .clone()
                    .unwrap_or_else(|| "Final phase: wounded and more dangerous.".to_owned()),
            );
        } else if boss.wounded_phase_hp_threshold.is_some() && alive {
            lines.push(
                boss.wounded_phase_inspect_pending
                    .clone()
                    .unwrap_or_else(|| "Final phase: not yet active.".to_owned()),
            );
        } else if boss.wounded_phase_hp_threshold.is_some() {
            lines.push(
                boss.wounded_phase_inspect_defeated
                    .clone()
                    .unwrap_or_else(|| "Final phase: no longer active.".to_owned()),
            );
        }
    }

    lines
}

fn sense_line(can_hear: bool, can_see: bool) -> String {
    format!(
        "Senses: hearing {} | sight {}",
        if can_hear { "yes" } else { "no" },
        if can_see { "yes" } else { "no" }
    )
}
