use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, GameEvent};
use super::context::{inspect_boss_state_lines, inspect_enemy_state_lines, item_context_lines};
use super::queries::{
    describe_current_location, describe_location, find_boss_by_name_or_id, find_enemy,
    find_enemy_by_name_or_id, find_item_by_name_or_id, find_location_by_name_or_id, matches_name,
    unlock_targets_for_item,
};
use super::state::RunState;

pub(super) fn handle_inspect(
    state: &RunState,
    bundle: &DatapackBundle,
    target: &str,
) -> ActionOutcome {
    if target == "room" || target == "area" || target == "location" {
        return ActionOutcome {
            events: vec![GameEvent::LocationLooked {
                location_id: state.current_location_id.clone(),
            }],
            lines: describe_current_location(state, bundle),
        };
    }

    if let Some(location) = find_location_by_name_or_id(bundle, target) {
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: location.id.clone(),
            }],
            lines: describe_location(state, bundle, location),
        };
    }

    if let Some(item) = find_item_by_name_or_id(bundle, target) {
        let unlock_targets = unlock_targets_for_item(bundle, &item.id);
        let mut lines = vec![format!("{}: {}", item.name, item.description)];
        if !unlock_targets.is_empty() {
            lines.push(format!("Can unlock: {}.", unlock_targets.join(", ")));
        }
        lines.extend(item_context_lines(state, item));
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: item.id.clone(),
            }],
            lines,
        };
    }

    if let Some(enemy_id) = state
        .location_enemies
        .get(&state.current_location_id)
        .and_then(|enemy_ids| {
            enemy_ids
                .iter()
                .rev()
                .find(|enemy_id| {
                    state.enemies_alive.contains(*enemy_id)
                        && find_enemy(bundle, enemy_id)
                            .is_some_and(|enemy| matches_name(target, &enemy.id, &enemy.name))
                })
                .cloned()
        })
        && let Some(enemy) = find_enemy(bundle, &enemy_id)
    {
        let mut lines = vec![format!("{}: {}", enemy.name, enemy.description)];
        lines.extend(inspect_enemy_state_lines(state, bundle, &enemy_id));
        return ActionOutcome {
            events: vec![GameEvent::Inspected { target: enemy_id }],
            lines,
        };
    }

    if let Some(enemy) = find_enemy_by_name_or_id(bundle, target) {
        let mut lines = vec![format!("{}: {}", enemy.name, enemy.description)];
        lines.extend(inspect_enemy_state_lines(state, bundle, &enemy.id));
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: enemy.id.clone(),
            }],
            lines,
        };
    }

    if let Some(boss) = find_boss_by_name_or_id(bundle, target) {
        let mut lines = vec![format!("{}: {}", boss.name, boss.description)];
        lines.extend(inspect_boss_state_lines(state, bundle, &boss.id));
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: boss.id.clone(),
            }],
            lines,
        };
    }

    ActionOutcome {
        events: vec![GameEvent::Inspected {
            target: target.to_owned(),
        }],
        lines: vec!["There is nothing useful to inspect by that name.".to_owned()],
    }
}
