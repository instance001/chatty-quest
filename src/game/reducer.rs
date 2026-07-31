use std::collections::{HashMap, HashSet, VecDeque};

use crate::data::datapacks::DatapackBundle;

use super::actions::{
    ActionOutcome, EncounterKind, GameAction, GameEvent, ItemUseEffect, MovementHazardKind,
};
use super::derived::{
    boss_wounded_phase_active, boss_wounded_phase_damage_bonus,
    finale_security_retaliation_reduction,
};
use super::queries::{
    describe_current_location, describe_location, equipped_damage, find_boss,
    find_boss_by_name_or_id, find_enemy, find_enemy_by_name_or_id, find_item,
    find_item_by_name_or_id, find_location, find_location_by_name_or_id, is_location_barricaded,
    is_location_locked, matches_name, unlock_targets_for_item,
};
use super::state::{InventoryEntry, RunState};

const ROLLING_SUMMARY_LIMIT: usize = 24;
const MAX_NOISE_LEVEL: i32 = 3;

pub fn apply_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: GameAction,
) -> ActionOutcome {
    if state.active_objective.completed {
        let outcome = apply_epilogue_action(state, bundle, action);
        advance_turn_if_accepted(state, &outcome);
        append_rolling_summary(state, &outcome);
        return outcome;
    }

    let was_alive = state.hp > 0;
    let action_for_noise = action.clone();
    let objective_before = objective_condition_statuses(state);
    let mut outcome = match action {
        GameAction::Help => ActionOutcome {
            events: vec![GameEvent::HelpShown],
            lines: vec![
                "Commands: help, look, go <location>, unlock <location>, barricade <location>, inspect <thing>, take <item>, equip <item>, use <item>, attack, wait."
                    .to_owned(),
            ],
        },
        GameAction::Look => ActionOutcome {
            events: vec![GameEvent::LocationLooked {
                location_id: state.current_location_id.clone(),
            }],
            lines: describe_current_location(state, bundle),
        },
        GameAction::Move { destination } => handle_move(state, bundle, &destination),
        GameAction::Unlock { target } => handle_unlock(state, bundle, &target),
        GameAction::Barricade { target } => handle_barricade(state, bundle, &target),
        GameAction::Inspect { target } => handle_inspect(state, bundle, &target),
        GameAction::Take { item_name } => handle_take(state, bundle, &item_name),
        GameAction::Equip { item_name } => handle_equip(state, &item_name),
        GameAction::Use { item_name } => handle_use(state, bundle, &item_name),
        GameAction::Attack => handle_attack(state, bundle),
        GameAction::Wait => handle_wait(state, bundle),
    };

    let accepted_turn = !action_was_rejected_or_blocked(&outcome);
    if accepted_turn {
        advance_turn_index(state);
    }

    apply_noise_for_action(state, bundle, &action_for_noise, &mut outcome);
    if state.hp > 0 && accepted_turn {
        apply_spawned_enemy_turns(state, bundle, &mut outcome);
    }

    let objective_after = objective_condition_statuses(state);
    outcome.lines.extend(objective_progress_lines(
        &objective_before,
        &objective_after,
    ));

    if update_objective_completion(state) {
        outcome.events.push(GameEvent::ObjectiveCompleted {
            objective_id: state.active_objective.id.clone(),
        });
        outcome.events.push(GameEvent::RunWon);
        outcome.lines.push(format!(
            "Objective complete: {}.",
            state.active_objective.name
        ));
        outcome.lines.push("You win.".to_owned());
    }

    if was_alive && state.hp <= 0 {
        outcome.events.push(GameEvent::RunLost);
        outcome.lines.push("You lose.".to_owned());
    }

    append_rolling_summary(state, &outcome);
    ActionOutcome {
        events: outcome.events,
        lines: outcome.lines,
    }
}

fn apply_epilogue_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: GameAction,
) -> ActionOutcome {
    match action {
        GameAction::Help => ActionOutcome {
            events: vec![GameEvent::HelpShown],
            lines: vec![
                "Epilogue commands: help, look, go <location>, inspect <thing>, equip <item>. Save and load remain available from the top bar."
                    .to_owned(),
                "The run is won, but the scenario can still be explored for aftermath, screenshots, and future datapack epilogue content."
                    .to_owned(),
            ],
        },
        GameAction::Look => ActionOutcome {
            events: vec![GameEvent::LocationLooked {
                location_id: state.current_location_id.clone(),
            }],
            lines: describe_current_location(state, bundle),
        },
        GameAction::Move { destination } => handle_move(state, bundle, &destination),
        GameAction::Inspect { target } => handle_inspect(state, bundle, &target),
        GameAction::Equip { item_name } => handle_equip(state, &item_name),
        GameAction::Attack => epilogue_rejection(
            "The run is already won. There is nothing left here that needs killing.",
        ),
        GameAction::Wait => epilogue_rejection(
            "The run is already won. You can linger, but the siege clock is no longer spending your HP.",
        ),
        GameAction::Take { .. } => epilogue_rejection(
            "The run is already won. Loot changes are paused for the epilogue pass.",
        ),
        GameAction::Use { .. } => epilogue_rejection(
            "The run is already won. Consumable and utility effects are paused for the epilogue pass.",
        ),
        GameAction::Unlock { .. } => epilogue_rejection(
            "The run is already won. Gate changes are paused for the epilogue pass.",
        ),
        GameAction::Barricade { .. } => epilogue_rejection(
            "The run is already won. Barricade changes are paused for the epilogue pass.",
        ),
    }
}

fn epilogue_rejection(line: &str) -> ActionOutcome {
    ActionOutcome {
        events: vec![GameEvent::ActionRejected {
            reason: line.to_owned(),
        }],
        lines: vec![line.to_owned()],
    }
}

fn handle_move(state: &mut RunState, bundle: &DatapackBundle, destination: &str) -> ActionOutcome {
    let Some(current_location) = find_location(bundle, &state.current_location_id) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Current location could not be resolved.".to_owned(),
            }],
            lines: vec!["Current location could not be resolved.".to_owned()],
        };
    };

    let Some(destination_location) = find_location_by_name_or_id(bundle, destination) else {
        return ActionOutcome {
            events: vec![GameEvent::MovementBlocked {
                attempted_destination: destination.to_owned(),
            }],
            lines: vec!["That destination is not part of this scenario.".to_owned()],
        };
    };

    if !current_location
        .connections
        .iter()
        .any(|connection| connection == &destination_location.id)
    {
        return ActionOutcome {
            events: vec![GameEvent::MovementBlocked {
                attempted_destination: destination_location.id.clone(),
            }],
            lines: vec![
                state
                    .boundary_response
                    .clone()
                    .unwrap_or_else(|| "You cannot get there from here.".to_owned()),
            ],
        };
    }

    if is_location_locked(state, &destination_location.id) {
        let locked_line = destination_location
            .locked_response
            .clone()
            .unwrap_or_else(|| {
                format!(
                    "The {} door is locked. You need the house keys.",
                    destination_location.name.to_ascii_lowercase()
                )
            });
        return ActionOutcome {
            events: vec![GameEvent::MovementBlocked {
                attempted_destination: destination_location.id.clone(),
            }],
            lines: vec![locked_line],
        };
    }

    let from_location_id = current_location.id.clone();
    state.current_location_id = destination_location.id.clone();
    state
        .known_locations
        .insert(destination_location.id.clone());
    state
        .visited_locations
        .insert(destination_location.id.clone());

    let mut lines = vec![
        format!("You move to {}.", destination_location.name),
        destination_location.description.clone(),
    ];
    lines.extend(movement_context_lines(state, bundle, destination_location));

    ActionOutcome {
        events: vec![GameEvent::Moved {
            from_location_id,
            to_location_id: destination_location.id.clone(),
        }],
        lines,
    }
}

fn handle_inspect(state: &RunState, bundle: &DatapackBundle, target: &str) -> ActionOutcome {
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

fn handle_take(state: &mut RunState, bundle: &DatapackBundle, item_name: &str) -> ActionOutcome {
    let Some(location_items) = state.location_items.get_mut(&state.current_location_id) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "This location has no item state.".to_owned(),
            }],
            lines: vec!["This location has no item state.".to_owned()],
        };
    };

    let Some(item_id) = location_items
        .iter()
        .find(|item_id| {
            find_item(bundle, item_id)
                .map(|item| matches_name(item_name, &item.id, &item.name))
                .unwrap_or(false)
        })
        .cloned()
    else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "There is no such item here to take.".to_owned(),
            }],
            lines: vec!["There is no such item here to take.".to_owned()],
        };
    };

    location_items.retain(|entry| entry != &item_id);

    let Some(item) = find_item(bundle, &item_id) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "The item data could not be resolved.".to_owned(),
            }],
            lines: vec!["The item data could not be resolved.".to_owned()],
        };
    };

    state.inventory.push(InventoryEntry {
        id: item.id.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        damage: item.damage,
    });

    ActionOutcome {
        events: vec![GameEvent::ItemTaken {
            item_id: item.id.clone(),
        }],
        lines: {
            let mut lines = vec![format!("You take the {}.", item.name)];
            if let Some(pickup_line) = item.pickup_line.as_deref() {
                lines.push(pickup_line.to_owned());
            }
            lines
        },
    }
}

fn handle_equip(state: &mut RunState, item_name: &str) -> ActionOutcome {
    let Some(item) = state
        .inventory
        .iter()
        .find(|item| matches_name(item_name, &item.id, &item.name))
        .cloned()
    else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You do not have that item.".to_owned(),
            }],
            lines: vec!["You do not have that item.".to_owned()],
        };
    };

    state.equipped_item_id = Some(item.id.clone());

    ActionOutcome {
        events: vec![GameEvent::ItemEquipped {
            item_id: item.id.clone(),
        }],
        lines: vec![format!("You equip the {}.", item.name)],
    }
}

fn handle_use(state: &mut RunState, bundle: &DatapackBundle, item_name: &str) -> ActionOutcome {
    let Some((index, item)) = state
        .inventory
        .iter()
        .enumerate()
        .find(|(_, item)| matches_name(item_name, &item.id, &item.name))
        .map(|(index, item)| (index, item.clone()))
    else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You do not have that item.".to_owned(),
            }],
            lines: vec!["You do not have that item.".to_owned()],
        };
    };

    let Some(template) = find_item(bundle, &item.id) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "The item data could not be resolved.".to_owned(),
            }],
            lines: vec!["The item data could not be resolved.".to_owned()],
        };
    };

    if let Some(outcome) = try_unlock_with_item(state, bundle, &item.id, None) {
        return outcome;
    }

    if template.utility_effect.as_deref() == Some("reveal_connections") {
        return reveal_connected_locations(state, bundle, &item);
    }

    if template.tags.iter().any(|tag| tag == "healing") {
        let previous_hp = state.hp;
        state.hp = (state.hp + 4).min(state.max_hp);
        state.inventory.remove(index);
        if state.equipped_item_id.as_deref() == Some(item.id.as_str()) {
            state.equipped_item_id = None;
        }

        return ActionOutcome {
            events: vec![GameEvent::ItemUsed {
                item_id: item.id.clone(),
                effect: ItemUseEffect::Healing {
                    amount: state.hp - previous_hp,
                },
            }],
            lines: vec![
                format!("You use the {}.", item.name),
                format!("HP rises from {} to {}.", previous_hp, state.hp),
                "You buy yourself a few ragged minutes of competence.".to_owned(),
            ],
        };
    }

    ActionOutcome {
        events: vec![GameEvent::ItemUsed {
            item_id: item.id.clone(),
            effect: ItemUseEffect::NoEffect,
        }],
        lines: vec![format!(
            "You fumble with the {}, but it has no usable v0.1 effect.",
            item.name
        )],
    }
}

fn reveal_connected_locations(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item: &InventoryEntry,
) -> ActionOutcome {
    let Some(current_location) = find_location(bundle, &state.current_location_id) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Current location could not be resolved.".to_owned(),
            }],
            lines: vec!["Current location could not be resolved.".to_owned()],
        };
    };

    let mut newly_known = Vec::new();
    for connection_id in &current_location.connections {
        if state.known_locations.insert(connection_id.clone()) {
            let location_name = find_location(bundle, connection_id)
                .map(|location| location.name.clone())
                .unwrap_or_else(|| connection_id.clone());
            newly_known.push(location_name);
        }
    }

    let item_template = find_item(bundle, &item.id);
    let mut lines = if newly_known.is_empty() {
        vec![
            item_template
                .and_then(|template| template.utility_empty_line.clone())
                .unwrap_or_else(|| {
                    format!("The {} does not reveal anything new from here.", item.name)
                }),
        ]
    } else {
        vec![
            item_template
                .and_then(|template| template.utility_success_line.clone())
                .unwrap_or_else(|| {
                    format!(
                        "You use the {} and get a better read on the nearby routes.",
                        item.name
                    )
                }),
            format!("Newly known routes: {}.", newly_known.join(", ")),
        ]
    };

    ActionOutcome {
        events: vec![GameEvent::ItemUsed {
            item_id: item.id.clone(),
            effect: ItemUseEffect::RevealedLocations {
                count: newly_known.len(),
            },
        }],
        lines: {
            if newly_known.is_empty() {
                lines
            } else {
                lines.push(format!("You use the {}.", item.name));
                lines
            }
        },
    }
}

fn handle_unlock(state: &mut RunState, bundle: &DatapackBundle, target: &str) -> ActionOutcome {
    let Some(location) = find_location_by_name_or_id(bundle, target) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "That location is not part of this scenario.".to_owned(),
            }],
            lines: vec!["That location is not part of this scenario.".to_owned()],
        };
    };

    let Some(unlock_item_id) = location.unlock_item_id.as_deref() else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "That location has no gate to unlock.".to_owned(),
            }],
            lines: vec!["That location has no gate to unlock.".to_owned()],
        };
    };

    if state.broken_locked_locations.contains(&location.id) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "That gate is already broken open.".to_owned(),
            }],
            lines: vec!["That gate is already broken open.".to_owned()],
        };
    }

    if !state.inventory.iter().any(|item| item.id == unlock_item_id) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: format!("You need {} to unlock {}.", unlock_item_id, location.name),
            }],
            lines: vec![format!(
                "You need {} to unlock {}.",
                unlock_item_id, location.name
            )],
        };
    }

    try_unlock_with_item(state, bundle, unlock_item_id, Some(&location.id)).unwrap_or(
        ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You cannot reach that gate from here.".to_owned(),
            }],
            lines: vec!["You cannot reach that gate from here.".to_owned()],
        },
    )
}

fn handle_barricade(state: &mut RunState, bundle: &DatapackBundle, target: &str) -> ActionOutcome {
    let Some(location) = find_location_by_name_or_id(bundle, target) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "That location is not part of this scenario.".to_owned(),
            }],
            lines: vec!["That location is not part of this scenario.".to_owned()],
        };
    };

    if !location.barricadable {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "That location cannot be barricaded.".to_owned(),
            }],
            lines: vec!["That location cannot be barricaded.".to_owned()],
        };
    }

    if state.current_location_id != location.id {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You need to be there to barricade it.".to_owned(),
            }],
            lines: vec!["You need to be there to barricade it.".to_owned()],
        };
    }

    if state.barricaded_locations.contains(&location.id) {
        let line = location
            .already_barricaded_response
            .clone()
            .unwrap_or_else(|| format!("{} is already barricaded.", location.name));
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: line.clone(),
            }],
            lines: vec![line],
        };
    }

    let Some(barricade_item_id) = location.barricade_item_id.as_deref() else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "This barricade rule is missing its required item.".to_owned(),
            }],
            lines: vec!["This barricade rule is missing its required item.".to_owned()],
        };
    };

    if !state
        .inventory
        .iter()
        .any(|item| item.id == barricade_item_id)
    {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You do not have the right materials.".to_owned(),
            }],
            lines: vec!["You do not have the right materials.".to_owned()],
        };
    }

    state.barricaded_locations.insert(location.id.clone());

    let previous_hp = state.hp;
    if location.barricade_heal > 0 {
        state.hp = (state.hp + location.barricade_heal).min(state.max_hp);
    }

    let mut lines = vec![
        location
            .barricade_response
            .clone()
            .unwrap_or_else(|| format!("You barricade {}.", location.name)),
    ];
    if state.hp > previous_hp {
        lines.push(format!(
            "You finally get a second to breathe. HP rises from {} to {}.",
            previous_hp, state.hp
        ));
    }

    ActionOutcome {
        events: vec![GameEvent::LocationBarricaded {
            location_id: location.id.clone(),
            item_id: barricade_item_id.to_owned(),
        }],
        lines,
    }
}

fn try_unlock_with_item(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item_id: &str,
    target_location_id: Option<&str>,
) -> Option<ActionOutcome> {
    let current_location = find_location(bundle, &state.current_location_id)?;

    let reachable_locked_locations = bundle
        .locations
        .iter()
        .filter(|location| location.locked)
        .filter(|location| state.locked_locations.contains(&location.id))
        .filter(|location| {
            location.id == current_location.id
                || current_location
                    .connections
                    .iter()
                    .any(|connection| connection == &location.id)
        })
        .collect::<Vec<_>>();

    if let Some(target_location_id) = target_location_id {
        let target = reachable_locked_locations
            .iter()
            .find(|location| location.id == target_location_id)?;
        if target.unlock_item_id.as_deref() != Some(item_id) {
            return Some(ActionOutcome {
                events: vec![GameEvent::ActionRejected {
                    reason: format!("{} does not unlock {}.", item_id, target.name),
                }],
                lines: vec![format!("{} does not unlock {}.", item_id, target.name)],
            });
        }
        return Some(unlock_location(state, target, item_id));
    }

    let matching_locations = reachable_locked_locations
        .into_iter()
        .filter(|location| location.unlock_item_id.as_deref() == Some(item_id))
        .collect::<Vec<_>>();

    if matching_locations.len() > 1 {
        let names = matching_locations
            .iter()
            .map(|location| location.name.clone())
            .collect::<Vec<_>>();
        return Some(ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: format!(
                    "More than one gate matches this item here: {}.",
                    names.join(", ")
                ),
            }],
            lines: vec![format!(
                "More than one gate matches this item here: {}. Use unlock <location>.",
                names.join(", ")
            )],
        });
    }

    if let Some(target) = matching_locations.into_iter().next() {
        return Some(unlock_location(state, target, item_id));
    }

    let has_any_matching_gate = bundle
        .locations
        .iter()
        .any(|location| location.unlock_item_id.as_deref() == Some(item_id));

    if has_any_matching_gate {
        return Some(ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "That unlock item does not help here.".to_owned(),
            }],
            lines: vec!["That unlock item does not help here.".to_owned()],
        });
    }

    None
}

fn unlock_location(
    state: &mut RunState,
    location: &crate::data::datapacks::LocationTemplate,
    item_id: &str,
) -> ActionOutcome {
    state.locked_locations.remove(&location.id);
    ActionOutcome {
        events: vec![
            GameEvent::ItemUsed {
                item_id: item_id.to_owned(),
                effect: ItemUseEffect::NoEffect,
            },
            GameEvent::LocationUnlocked {
                location_id: location.id.clone(),
                item_id: item_id.to_owned(),
            },
        ],
        lines: vec![format!(
            "You unlock {} with {}.",
            location.name,
            item_id.replace('_', " ")
        )],
    }
}

fn handle_attack(state: &mut RunState, bundle: &DatapackBundle) -> ActionOutcome {
    let current_location = state.current_location_id.clone();

    let enemy_here = state
        .location_enemies
        .get(&current_location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| state.enemies_alive.contains(*id))
                .cloned()
        });
    let boss_here = state
        .location_bosses
        .get(&current_location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| state.bosses_alive.contains(*id))
                .cloned()
        });

    if let Some(enemy_id) = enemy_here {
        let barricade_attack_bonus =
            find_location(bundle, &current_location).map_or(0, |location| {
                if state.barricaded_locations.contains(&location.id) {
                    location.barricade_attack_bonus
                } else {
                    0
                }
            });
        let player_damage = (equipped_damage(state) + barricade_attack_bonus).max(1);
        let enemy_damage = state.enemy_hp.entry(enemy_id.clone()).or_insert(0);
        *enemy_damage -= player_damage;

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        if barricade_attack_bonus > 0 {
            lines.push(format!(
                "The barricade gives you a steadier angle on the threat. Attack bonus: +{}.",
                barricade_attack_bonus
            ));
        }
        let mut events = vec![GameEvent::AttackResolved {
            target_id: enemy_id.clone(),
            target_kind: EncounterKind::Enemy,
            damage: player_damage,
            defeated: *enemy_damage <= 0,
        }];

        if *enemy_damage <= 0 {
            state.enemies_alive.remove(&enemy_id);
            state.enemies_defeated.insert(enemy_id.clone());
            state.spawned_enemy_targets.remove(&enemy_id);
            state.spawned_enemy_origins.remove(&enemy_id);
            state.spawned_enemy_searching.remove(&enemy_id);
            state.spawned_enemy_sight_targets.remove(&enemy_id);
            state.spawned_enemy_sight_subjects.remove(&enemy_id);
            state.spawned_enemy_sight_delays.remove(&enemy_id);
            if let Some(entries) = state.location_enemies.get_mut(&current_location) {
                entries.retain(|entry| entry != &enemy_id);
            }
            let enemy_name = find_enemy(bundle, &enemy_id)
                .map(|enemy| enemy.name.clone())
                .unwrap_or_else(|| enemy_id.clone());
            lines.push(format!("{} goes down.", enemy_name));
            if let Some(defeat_line) =
                find_enemy(bundle, &enemy_id).and_then(|enemy| enemy.defeat_line.clone())
            {
                lines.push(defeat_line);
            }
        } else {
            let retaliation_blocked =
                find_location(bundle, &current_location).is_some_and(|location| {
                    location.barricade_blocks_retaliation
                        && state.barricaded_locations.contains(&location.id)
                });

            if retaliation_blocked {
                lines.push(
                    "The barricade keeps the threat at splinter-spitting distance. It cannot land the hit cleanly."
                        .to_owned(),
                );
            } else {
                let retaliation_bonus =
                    exposed_noise_retaliation_bonus(state, bundle, &current_location);
                let retaliation = find_enemy(bundle, &enemy_id)
                    .map(|enemy| enemy.damage)
                    .unwrap_or(1)
                    + retaliation_bonus;
                state.hp = (state.hp - retaliation).max(0);
                events.push(GameEvent::DamageTaken {
                    amount: retaliation,
                    remaining_hp: state.hp,
                });
                lines.push(format!("The enemy hits back for {} damage.", retaliation));
                if let Some(retaliation_line) =
                    find_enemy(bundle, &enemy_id).and_then(|enemy| enemy.retaliation_line.clone())
                {
                    lines.push(retaliation_line);
                }
                lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
            }
        }

        return ActionOutcome { events, lines };
    }

    if let Some(boss_id) = boss_here {
        let player_damage = equipped_damage(state).max(1);
        let boss_template = find_boss(bundle, &boss_id);
        let boss_damage = state.boss_hp.entry(boss_id.clone()).or_insert(0);
        *boss_damage -= player_damage;
        let boss_remaining_hp = *boss_damage;
        let wounded_phase =
            boss_template.is_some_and(|boss| boss_wounded_phase_active(boss, boss_remaining_hp));

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        let mut events = vec![GameEvent::AttackResolved {
            target_id: boss_id.clone(),
            target_kind: EncounterKind::Boss,
            damage: player_damage,
            defeated: boss_remaining_hp <= 0,
        }];

        if boss_remaining_hp <= 0 {
            state.bosses_alive.remove(&boss_id);
            state.bosses_defeated.insert(boss_id.clone());
            if let Some(entries) = state.location_bosses.get_mut(&current_location) {
                entries.retain(|entry| entry != &boss_id);
            }
            let boss_name = find_boss(bundle, &boss_id)
                .map(|boss| boss.name.clone())
                .unwrap_or_else(|| boss_id.clone());
            lines.push(
                boss_template
                    .and_then(|boss| boss.defeat_line.as_deref())
                    .map(|line| render_boss_combat_line(line, &boss_name, player_damage, 0))
                    .unwrap_or_else(|| {
                        format!(
                            "{} collapses. The worst thing on the block is finished.",
                            boss_name
                        )
                    }),
            );
            if state.active_objective.required_location_id.as_deref()
                == Some(state.current_location_id.as_str())
                && let Some(line) = find_location(bundle, &state.current_location_id)
                    .and_then(|location| location.boss_defeated_objective_line.as_deref())
            {
                lines.push(line.to_owned());
            }
        } else {
            if wounded_phase {
                lines.push(
                    boss_template
                        .and_then(|boss| boss.wounded_phase_combat_line.clone())
                        .unwrap_or_else(|| {
                            "The boss enters a wounded final phase and becomes more dangerous."
                                .to_owned()
                        }),
                );
            }
            let secured_property_bonus =
                finale_security_retaliation_reduction(state, bundle, &boss_id);
            let wounded_bonus = boss_template
                .map(boss_wounded_phase_damage_bonus)
                .filter(|_| wounded_phase)
                .unwrap_or(0);
            let retaliation = (boss_template.map(|boss| boss.damage).unwrap_or(2) + wounded_bonus
                - secured_property_bonus)
                .max(1);
            state.hp = (state.hp - retaliation).max(0);
            events.push(GameEvent::DamageTaken {
                amount: retaliation,
                remaining_hp: state.hp,
            });
            let boss_name = boss_template
                .map(|boss| boss.name.as_str())
                .unwrap_or("The boss");
            lines.push(
                boss_template
                    .and_then(|boss| boss.retaliation_line.as_deref())
                    .map(|line| render_boss_combat_line(line, boss_name, retaliation, 0))
                    .unwrap_or_else(|| {
                        format!("The boss smashes back for {} damage.", retaliation)
                    }),
            );
            if secured_property_bonus > 0
                && let Some(line) =
                    boss_template.and_then(|boss| boss.finale_security_retaliation_line.as_deref())
            {
                lines.push(render_boss_combat_line(
                    line,
                    boss_name,
                    retaliation,
                    secured_property_bonus,
                ));
            }
            if let Some(line) = find_location(bundle, &state.current_location_id)
                .and_then(|location| location.boss_retaliation_context_line.as_deref())
            {
                lines.push(line.to_owned());
            }
            if wounded_phase {
                lines.push(
                    boss_template
                        .and_then(|boss| boss.wounded_phase_retaliation_line.clone())
                        .unwrap_or_else(|| {
                            "Final-phase pressure: the boss is hitting harder now.".to_owned()
                        }),
                );
            }
            lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
        }

        return ActionOutcome { events, lines };
    }

    ActionOutcome {
        events: vec![GameEvent::AttackWhiff],
        lines: vec!["You swing at the air with admirable commitment.".to_owned()],
    }
}

fn handle_wait(state: &mut RunState, bundle: &DatapackBundle) -> ActionOutcome {
    let location = find_location(bundle, &state.current_location_id);
    let location_name = location
        .as_ref()
        .map(|location| location.name.clone())
        .unwrap_or_else(|| state.current_location_id.clone());

    let mut events = vec![GameEvent::Waited {
        location_id: state.current_location_id.clone(),
    }];
    let mut lines = vec![format!(
        "You wait at {} and listen to the property complain around you.",
        location_name
    )];

    if let Some(location) = location.as_ref() {
        apply_location_passive_pressure(state, location, &mut events, &mut lines);
    }

    ActionOutcome { events, lines }
}

fn apply_location_passive_pressure(
    state: &mut RunState,
    location: &crate::data::datapacks::LocationTemplate,
    events: &mut Vec<GameEvent>,
    lines: &mut Vec<String>,
) {
    let Some(enemy_id) = location.passive_pressure_enemy_id.as_deref() else {
        return;
    };
    if !state.enemies_alive.contains(enemy_id) {
        return;
    }

    if is_location_barricaded(state, &location.id) {
        if let Some(line) = location.passive_pressure_blocked_line.as_deref() {
            lines.push(line.to_owned());
        }
        return;
    }

    let pressure = exposed_noise_pressure_damage(state);
    state.hp = (state.hp - pressure).max(0);
    events.push(GameEvent::DamageTaken {
        amount: pressure,
        remaining_hp: state.hp,
    });
    if let Some(line) = location.passive_pressure_damage_line.as_deref() {
        lines.push(line.to_owned());
    }
    if pressure > 1
        && let Some(line) = location.passive_pressure_high_noise_line.as_deref()
    {
        lines.push(line.to_owned());
    }
    lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
}

fn render_boss_combat_line(line: &str, boss_name: &str, damage: i32, reduction: i32) -> String {
    line.replace("{boss_name}", boss_name)
        .replace("{damage}", &damage.to_string())
        .replace("{reduction}", &reduction.to_string())
}

fn apply_noise_for_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: &GameAction,
    outcome: &mut ActionOutcome,
) {
    if action_was_rejected_or_blocked(outcome) {
        return;
    }

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

fn action_was_rejected_or_blocked(outcome: &ActionOutcome) -> bool {
    outcome.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ActionRejected { .. } | GameEvent::MovementBlocked { .. }
        )
    })
}

fn advance_turn_if_accepted(state: &mut RunState, outcome: &ActionOutcome) {
    if !action_was_rejected_or_blocked(outcome) {
        advance_turn_index(state);
    }
}

fn advance_turn_index(state: &mut RunState) {
    state.turn_index = state.turn_index.saturating_add(1);
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

fn apply_spawned_enemy_turns(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
) {
    let spawned_this_turn = outcome
        .events
        .iter()
        .filter_map(|event| {
            if let GameEvent::NoiseSpawnedEnemy { enemy_id, .. } = event {
                Some(enemy_id.clone())
            } else {
                None
            }
        })
        .collect::<HashSet<_>>();
    let mut spawned_ids = state
        .enemies_alive
        .iter()
        .filter(|enemy_id| is_noise_spawned_enemy(enemy_id))
        .cloned()
        .collect::<Vec<_>>();
    spawned_ids.sort();

    for enemy_id in spawned_ids {
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
            state.spawned_enemy_searching.insert(enemy_id.clone());
        }
        let is_searching = state.spawned_enemy_searching.contains(&enemy_id);

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
            state.spawned_enemy_sight_delays.insert(enemy_id.clone(), 1);
            step = SpawnedEnemyStep::Wait(format!(
                "chasing {} by sight takes an extra moment",
                state
                    .spawned_enemy_sight_subjects
                    .get(&enemy_id)
                    .map(String::as_str)
                    .unwrap_or("the attractor")
            ));
        } else if sighted_this_turn {
            state.spawned_enemy_sight_delays.remove(&enemy_id);
        }

        match step {
            SpawnedEnemyStep::Move {
                to_location_id,
                step_target_location_id,
            } => {
                move_spawned_enemy(state, &enemy_id, &current_location_id, &to_location_id);
                let event_target_location_id =
                    step_target_location_id.unwrap_or_else(|| target_location_id.clone());
                outcome.events.push(GameEvent::SpawnedEnemyMoved {
                    enemy_id: enemy_id.clone(),
                    from_location_id: current_location_id.clone(),
                    to_location_id: to_location_id.clone(),
                    target_location_id: event_target_location_id,
                });
                outcome.lines.push(format!(
                    "{} shifts from {} toward {}.",
                    enemy_display_name(bundle, &enemy_id),
                    location_display_name(bundle, &current_location_id),
                    location_display_name(bundle, &to_location_id)
                ));
            }
            SpawnedEnemyStep::Wait(reason) => {
                outcome.events.push(GameEvent::SpawnedEnemyWaited {
                    enemy_id: enemy_id.clone(),
                    location_id: current_location_id.clone(),
                    reason: reason.clone(),
                });
                outcome.lines.push(format!(
                    "{} waits at {}: {}.",
                    enemy_display_name(bundle, &enemy_id),
                    location_display_name(bundle, &current_location_id),
                    reason
                ));
            }
            SpawnedEnemyStep::AttackHazard(hazard) => {
                let break_chance_percent = spawned_hazard_break_chance_percent(bundle);
                let roll_percent =
                    deterministic_hazard_break_roll(state, &enemy_id, &hazard.location_id);
                let broken = roll_percent < break_chance_percent;
                if broken {
                    break_movement_hazard(state, &hazard);
                }
                outcome.events.push(GameEvent::SpawnedEnemyAttackedHazard {
                    enemy_id: enemy_id.clone(),
                    hazard_kind: hazard.kind.clone(),
                    location_id: hazard.location_id.clone(),
                    break_chance_percent,
                    roll_percent,
                    broken,
                });
                outcome.lines.push(spawned_enemy_hazard_attack_line(
                    bundle, &enemy_id, &hazard, broken,
                ));
            }
        }
    }
}

enum SpawnedEnemyStep {
    Move {
        to_location_id: String,
        step_target_location_id: Option<String>,
    },
    Wait(String),
    AttackHazard(MovementHazard),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SpawnedEnemyAttractorKind {
    Sight,
    Noise,
    SearchFallback,
}

struct SpawnedEnemyAttractor {
    kind: SpawnedEnemyAttractorKind,
    location_id: String,
}

struct MovementHazard {
    kind: MovementHazardKind,
    location_id: String,
}

#[derive(Clone)]
struct SightAttractor {
    subject_id: String,
    location_id: String,
}

fn spawned_enemy_active_attractor(state: &RunState, enemy_id: &str) -> SpawnedEnemyAttractor {
    if let Some(location_id) = state.spawned_enemy_sight_targets.get(enemy_id) {
        return SpawnedEnemyAttractor {
            kind: SpawnedEnemyAttractorKind::Sight,
            location_id: location_id.clone(),
        };
    }

    if let Some(location_id) = state.spawned_enemy_targets.get(enemy_id) {
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

fn refresh_spawned_enemy_sight_target(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
    enemy_id: &str,
    current_location_id: &str,
) -> bool {
    if !spawned_enemy_can_see(bundle, enemy_id) {
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
                enemy_id: enemy_id.to_owned(),
                subject_id: attractor.subject_id.clone(),
                location_id: attractor.location_id.clone(),
                detect_chance_percent,
                roll_percent,
            });
            outcome.lines.push(format!(
                "{} {}.",
                enemy_display_name(bundle, enemy_id),
                sight_miss_flavor_line(state, enemy_id, &attractor.subject_id)
            ));
            clear_spawned_enemy_sight_to_search(state, bundle, outcome, enemy_id);
            return false;
        }

        let target_changed = state
            .spawned_enemy_sight_targets
            .get(enemy_id)
            .is_none_or(|location_id| location_id != &attractor.location_id)
            || state
                .spawned_enemy_sight_subjects
                .get(enemy_id)
                .is_none_or(|subject_id| subject_id != &attractor.subject_id);
        state
            .spawned_enemy_sight_targets
            .insert(enemy_id.to_owned(), attractor.location_id.clone());
        state
            .spawned_enemy_sight_subjects
            .insert(enemy_id.to_owned(), attractor.subject_id.clone());
        state.spawned_enemy_searching.remove(enemy_id);
        if target_changed {
            outcome.events.push(GameEvent::SightAttractorAcquired {
                enemy_id: enemy_id.to_owned(),
                subject_id: attractor.subject_id.clone(),
                location_id: attractor.location_id.clone(),
            });
            outcome.lines.push(format!(
                "{} catches sight of {} at {}.",
                enemy_display_name(bundle, enemy_id),
                subject_display_name(bundle, &attractor.subject_id),
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
    if state.spawned_enemy_sight_targets.contains_key(enemy_id)
        || state.spawned_enemy_sight_delays.contains_key(enemy_id)
    {
        state.spawned_enemy_sight_delays.remove(enemy_id);
        let subject_id = state
            .spawned_enemy_sight_subjects
            .remove(enemy_id)
            .unwrap_or_else(|| "the last seen attractor".to_owned());
        state.spawned_enemy_sight_targets.remove(enemy_id);
        state.spawned_enemy_searching.insert(enemy_id.to_owned());
        outcome.events.push(GameEvent::SightAttractorLost {
            enemy_id: enemy_id.to_owned(),
            subject_id: subject_id.clone(),
        });
        outcome.lines.push(format!(
            "{} {}.",
            enemy_display_name(bundle, enemy_id),
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
    if spawned_enemy_can_see_location(
        state,
        bundle,
        current_location_id,
        &state.current_location_id,
    ) {
        return Some(SightAttractor {
            subject_id: "player".to_owned(),
            location_id: state.current_location_id.clone(),
        });
    }

    if state.spawned_enemy_targets.contains_key(enemy_id)
        && !state.spawned_enemy_searching.contains(enemy_id)
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

fn spawned_enemy_next_step(
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
    let index = deterministic_noise_index(
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
    match deterministic_noise_index(
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
    let index = deterministic_noise_index(
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
    let Some(origin_location_id) = state.spawned_enemy_origins.get(enemy_id) else {
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

fn deterministic_hazard_break_roll(
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

fn spawned_hazard_break_chance_percent(bundle: &DatapackBundle) -> u8 {
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

fn sight_chase_should_delay(
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

fn break_movement_hazard(state: &mut RunState, hazard: &MovementHazard) {
    match hazard.kind {
        MovementHazardKind::Barricade => {
            state.barricaded_locations.remove(&hazard.location_id);
        }
        MovementHazardKind::LockedGate => {
            state.locked_locations.remove(&hazard.location_id);
            state
                .broken_locked_locations
                .insert(hazard.location_id.clone());
        }
    }
}

fn spawned_enemy_hazard_attack_line(
    bundle: &DatapackBundle,
    enemy_id: &str,
    hazard: &MovementHazard,
    broken: bool,
) -> String {
    let enemy_name = enemy_display_name(bundle, enemy_id);
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

fn move_spawned_enemy(
    state: &mut RunState,
    enemy_id: &str,
    from_location_id: &str,
    to_location_id: &str,
) {
    if let Some(entries) = state.location_enemies.get_mut(from_location_id) {
        entries.retain(|entry| entry != enemy_id);
    }
    let destination = state
        .location_enemies
        .entry(to_location_id.to_owned())
        .or_default();
    if !destination.iter().any(|entry| entry == enemy_id) {
        destination.push(enemy_id.to_owned());
    }
}

fn spawned_enemy_location(state: &RunState, enemy_id: &str) -> Option<String> {
    state
        .location_enemies
        .iter()
        .find(|(_, enemy_ids)| enemy_ids.iter().any(|entry| entry == enemy_id))
        .map(|(location_id, _)| location_id.clone())
}

fn is_noise_spawned_enemy(enemy_id: &str) -> bool {
    enemy_id.starts_with("noise_spawn_")
}

fn spawned_enemy_can_hear(bundle: &DatapackBundle, enemy_id: &str) -> bool {
    find_enemy(bundle, enemy_id).is_some_and(|enemy| enemy.can_hear)
}

fn spawned_enemy_can_see(bundle: &DatapackBundle, enemy_id: &str) -> bool {
    find_enemy(bundle, enemy_id).is_some_and(|enemy| enemy.can_see)
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
    deterministic_noise_index(state, len, enemy_search_salt(enemy_id, context_id, salt))
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

fn enemy_display_name(bundle: &DatapackBundle, enemy_id: &str) -> String {
    find_enemy(bundle, enemy_id)
        .map(|enemy| enemy.name.clone())
        .unwrap_or_else(|| enemy_id.to_owned())
}

fn subject_display_name(bundle: &DatapackBundle, subject_id: &str) -> String {
    if subject_id == "player" {
        return "you".to_owned();
    }
    find_enemy(bundle, subject_id)
        .map(|enemy| enemy.name.clone())
        .or_else(|| find_boss(bundle, subject_id).map(|boss| boss.name.clone()))
        .unwrap_or_else(|| subject_id.to_owned())
}

fn location_display_name(bundle: &DatapackBundle, location_id: &str) -> String {
    find_location(bundle, location_id)
        .map(|location| location.name.clone())
        .unwrap_or_else(|| location_id.to_owned())
}

fn location_display_name_for_state_only(location_id: &str) -> String {
    location_id.replace('_', " ")
}

fn select_noise_spawn_enemy(
    state: &RunState,
    bundle: &DatapackBundle,
) -> Option<crate::data::datapacks::EnemyTemplate> {
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
) -> Option<crate::data::datapacks::LocationTemplate> {
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

fn deterministic_noise_index(state: &RunState, len: usize, salt: usize) -> usize {
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

fn exposed_noise_pressure_damage(state: &RunState) -> i32 {
    if state.noise_level >= 2 { 2 } else { 1 }
}

fn exposed_noise_retaliation_bonus(
    state: &RunState,
    bundle: &DatapackBundle,
    location_id: &str,
) -> i32 {
    let Some(location) = find_location(bundle, location_id) else {
        return 0;
    };
    if state.noise_level >= 2
        && !state.barricaded_locations.contains(location_id)
        && location.tags.iter().any(|tag| tag == "noise_pressure")
    {
        1
    } else {
        0
    }
}

fn update_objective_completion(state: &mut RunState) -> bool {
    let boss_condition_met = state
        .active_objective
        .target_boss_id
        .as_ref()
        .is_none_or(|boss_id| state.bosses_defeated.contains(boss_id));
    let item_condition_met = state
        .active_objective
        .required_item_id
        .as_ref()
        .is_none_or(|item_id| state.inventory.iter().any(|item| &item.id == item_id));
    let location_condition_met = state
        .active_objective
        .required_location_id
        .as_ref()
        .is_none_or(|location_id| &state.current_location_id == location_id);
    let has_any_condition = state.active_objective.target_boss_id.is_some()
        || state.active_objective.required_item_id.is_some()
        || state.active_objective.required_location_id.is_some();
    let completed_now =
        has_any_condition && boss_condition_met && item_condition_met && location_condition_met;
    let just_completed = completed_now && !state.active_objective.completed;
    state.active_objective.completed = completed_now;
    just_completed
}

fn movement_context_lines(
    state: &RunState,
    bundle: &DatapackBundle,
    location: &crate::data::datapacks::LocationTemplate,
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

fn item_context_lines(
    state: &RunState,
    item: &crate::data::datapacks::ItemTemplate,
) -> Vec<String> {
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

fn inspect_enemy_state_lines(
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

fn inspect_boss_state_lines(
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

#[derive(Clone, Eq, PartialEq)]
struct ObjectiveConditionStatus {
    label: String,
    met: bool,
}

fn objective_condition_statuses(state: &RunState) -> Vec<ObjectiveConditionStatus> {
    let mut statuses = Vec::new();

    if let Some(required_item_id) = &state.active_objective.required_item_id {
        statuses.push(ObjectiveConditionStatus {
            label: format!("Hold item '{}'", required_item_id),
            met: state
                .inventory
                .iter()
                .any(|item| &item.id == required_item_id),
        });
    }

    if let Some(target_boss_id) = &state.active_objective.target_boss_id {
        statuses.push(ObjectiveConditionStatus {
            label: format!("Defeat boss '{}'", target_boss_id),
            met: state.bosses_defeated.contains(target_boss_id),
        });
    }

    if let Some(required_location_id) = &state.active_objective.required_location_id {
        statuses.push(ObjectiveConditionStatus {
            label: format!("Reach location '{}'", required_location_id),
            met: state.current_location_id == *required_location_id,
        });
    }

    statuses
}

fn objective_progress_lines(
    before: &[ObjectiveConditionStatus],
    after: &[ObjectiveConditionStatus],
) -> Vec<String> {
    after
        .iter()
        .filter_map(|after_status| {
            let before_status = before
                .iter()
                .find(|status| status.label == after_status.label)?;
            if before_status.met == after_status.met {
                return None;
            }

            Some(format!(
                "Objective progress: {} is now {}.",
                after_status.label,
                if after_status.met {
                    "complete"
                } else {
                    "incomplete"
                }
            ))
        })
        .collect()
}

fn rolling_summary_lines(events: &[GameEvent], fallback_lines: &[String]) -> Vec<String> {
    let mut lines = events
        .iter()
        .filter_map(summarize_event)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        lines.extend(fallback_lines.iter().cloned());
    }

    lines
}

fn append_rolling_summary(state: &mut RunState, outcome: &ActionOutcome) {
    let summary_lines = rolling_summary_lines(&outcome.events, &outcome.lines);
    state.rolling_summary.extend(summary_lines);
    trim_rolling_summary(&mut state.rolling_summary);
}

fn trim_rolling_summary(summary: &mut Vec<String>) {
    if summary.len() > ROLLING_SUMMARY_LIMIT {
        let remove_count = summary.len() - ROLLING_SUMMARY_LIMIT;
        summary.drain(0..remove_count);
    }
}

fn summarize_event(event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::HelpShown => Some("Help was shown.".to_owned()),
        GameEvent::ActionRejected { reason } => Some(format!("Action rejected: {}", reason)),
        GameEvent::LocationLooked { location_id } => {
            Some(format!("Looked around location '{}'.", location_id))
        }
        GameEvent::Moved {
            from_location_id,
            to_location_id,
        } => Some(format!(
            "Moved from '{}' to '{}'.",
            from_location_id, to_location_id
        )),
        GameEvent::MovementBlocked {
            attempted_destination,
        } => Some(format!(
            "Movement toward '{}' was blocked.",
            attempted_destination
        )),
        GameEvent::LocationUnlocked {
            location_id,
            item_id,
        } => Some(format!(
            "Unlocked location '{}' with item '{}'.",
            location_id, item_id
        )),
        GameEvent::LocationBarricaded {
            location_id,
            item_id,
        } => Some(format!(
            "Barricaded location '{}' with item '{}'.",
            location_id, item_id
        )),
        GameEvent::Inspected { target } => Some(format!("Inspected '{}'.", target)),
        GameEvent::ItemTaken { item_id } => Some(format!("Took item '{}'.", item_id)),
        GameEvent::ItemEquipped { item_id } => Some(format!("Equipped item '{}'.", item_id)),
        GameEvent::ItemUsed { item_id, effect } => match effect {
            ItemUseEffect::Healing { amount } => Some(format!(
                "Used item '{}' for healing {} HP.",
                item_id, amount
            )),
            ItemUseEffect::RevealedLocations { count } => Some(format!(
                "Used item '{}' to reveal {} connected location(s).",
                item_id, count
            )),
            ItemUseEffect::NoEffect => Some(format!("Used item '{}' with no effect.", item_id)),
        },
        GameEvent::AttackResolved {
            target_id,
            target_kind,
            damage,
            defeated,
        } => Some(format!(
            "Attack hit {:?} '{}' for {} damage{}.",
            target_kind,
            target_id,
            damage,
            if *defeated { " and defeated it" } else { "" }
        )),
        GameEvent::DamageTaken {
            amount,
            remaining_hp,
        } => Some(format!(
            "Took {} damage and dropped to {} HP.",
            amount, remaining_hp
        )),
        GameEvent::NoiseSpawnedEnemy {
            enemy_id,
            template_id,
            location_id,
        } => Some(format!(
            "Noise spawned enemy '{}' from template '{}' at '{}'.",
            enemy_id, template_id, location_id
        )),
        GameEvent::NoiseAttractorShifted {
            location_id,
            enemy_ids,
        } => Some(format!(
            "Noise attractor shifted to '{}' for spawned enemies: {}.",
            location_id,
            enemy_ids.join(", ")
        )),
        GameEvent::SightAttractorAcquired {
            enemy_id,
            subject_id,
            location_id,
        } => Some(format!(
            "Spawned enemy '{}' sighted '{}' at '{}'.",
            enemy_id, subject_id, location_id
        )),
        GameEvent::SightAttractorMissed {
            enemy_id,
            subject_id,
            location_id,
            detect_chance_percent,
            roll_percent,
        } => Some(format!(
            "Spawned enemy '{}' missed sighting '{}' at '{}' with {}% detect chance and roll {}.",
            enemy_id, subject_id, location_id, detect_chance_percent, roll_percent
        )),
        GameEvent::SightAttractorLost {
            enemy_id,
            subject_id,
        } => Some(format!(
            "Spawned enemy '{}' lost sight of '{}'.",
            enemy_id, subject_id
        )),
        GameEvent::SpawnedEnemyMoved {
            enemy_id,
            from_location_id,
            to_location_id,
            target_location_id,
        } => Some(format!(
            "Spawned enemy '{}' moved from '{}' to '{}' toward '{}'.",
            enemy_id, from_location_id, to_location_id, target_location_id
        )),
        GameEvent::SpawnedEnemyWaited {
            enemy_id,
            location_id,
            reason,
        } => Some(format!(
            "Spawned enemy '{}' waited at '{}': {}.",
            enemy_id, location_id, reason
        )),
        GameEvent::SpawnedEnemyAttackedHazard {
            enemy_id,
            hazard_kind,
            location_id,
            break_chance_percent,
            roll_percent,
            broken,
        } => Some(format!(
            "Spawned enemy '{}' attacked {:?} at '{}' with {}% break chance and rolled {}: broken={}.",
            enemy_id, hazard_kind, location_id, break_chance_percent, roll_percent, broken
        )),
        GameEvent::AttackWhiff => Some("Attack missed or found no target.".to_owned()),
        GameEvent::Waited { location_id } => Some(format!("Waited at '{}'.", location_id)),
        GameEvent::ObjectiveCompleted { objective_id } => {
            Some(format!("Completed objective '{}'.", objective_id))
        }
        GameEvent::RunWon => Some("Run won.".to_owned()),
        GameEvent::RunLost => Some("Run lost.".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{
        EncounterKind, GameAction, GameEvent, ItemUseEffect, MovementHazardKind,
    };
    use crate::game::generation::generate_new_run;

    use super::{
        ROLLING_SUMMARY_LIMIT, SpawnedEnemyStep, advance_turn_index, apply_action,
        deterministic_hazard_break_roll, spawned_enemy_next_step,
    };

    #[test]
    fn invalid_movement_is_blocked_by_boundary_rules() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        assert_eq!(state.current_location_id, "front_verandah");
        assert!(matches!(
            outcome.events.first(),
            Some(GameEvent::MovementBlocked { attempted_destination }) if attempted_destination == "laundry"
        ));
        assert_eq!(
            outcome.lines.first().map(String::as_str),
            Some("You make it three fences before the horde eats you, idiot.")
        );
    }

    #[test]
    fn accepted_actions_advance_turn_index_but_blocked_actions_do_not() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let look = apply_action(&mut state, &bundle, GameAction::Look);
        assert!(look.events.iter().any(|event| {
            matches!(event, GameEvent::LocationLooked { location_id } if location_id == "front_verandah")
        }));
        assert_eq!(state.turn_index, 1);

        let blocked = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        assert!(
            blocked
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::MovementBlocked { .. }))
        );
        assert_eq!(state.turn_index, 1);
    }

    #[test]
    fn station_smoke_test_pack_can_generate_move_fight_and_win() {
        let bundle = load_datapack_bundle_by_folder("station_smoke_test")
            .expect("expected station_smoke_test bundle to load");
        let mut state = generate_new_run(&bundle).state;

        assert_eq!(state.datapack_id, "station_smoke_test");
        assert_eq!(state.current_location_id, "station_platform");
        assert_eq!(
            state.active_objective.required_item_id.as_deref(),
            Some("brass_token")
        );

        let take = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "brass_token".to_owned(),
            },
        );
        assert!(take.events.iter().any(|event| {
            matches!(event, GameEvent::ItemTaken { item_id } if item_id == "brass_token")
        }));

        let move_to_signal_box = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "signal_box".to_owned(),
            },
        );
        assert!(move_to_signal_box.events.iter().any(|event| {
            matches!(event, GameEvent::Moved { to_location_id, .. } if to_location_id == "signal_box")
        }));

        let clear_guard = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(clear_guard.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AttackResolved {
                    target_id,
                    target_kind: EncounterKind::Enemy,
                    defeated: true,
                    ..
                } if target_id == "static_guard"
            )
        }));

        let hit_wraith = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(hit_wraith.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AttackResolved {
                    target_id,
                    target_kind: EncounterKind::Boss,
                    defeated: false,
                    ..
                } if target_id == "ticket_wraith"
            )
        }));

        let defeat_wraith = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(defeat_wraith.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AttackResolved {
                    target_id,
                    target_kind: EncounterKind::Boss,
                    defeated: true,
                    ..
                } if target_id == "ticket_wraith"
            )
        }));
        assert!(
            defeat_wraith
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
        assert!(
            defeat_wraith
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::RunWon))
        );
        assert!(state.active_objective.completed);
    }

    #[test]
    fn deterministic_rolls_advance_after_rolling_summary_is_capped() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_spawn_count = 1;
        state.noise_level = 3;
        state.rolling_summary = (0..ROLLING_SUMMARY_LIMIT)
            .map(|index| format!("summary {}", index))
            .collect();
        state.turn_index = ROLLING_SUMMARY_LIMIT as u64;

        let first_roll =
            deterministic_hazard_break_roll(&state, "noise_spawn_1_crawler_in_weeds", "garage");
        advance_turn_index(&mut state);
        let second_roll =
            deterministic_hazard_break_roll(&state, "noise_spawn_1_crawler_in_weeds", "garage");

        assert_eq!(state.rolling_summary.len(), ROLLING_SUMMARY_LIMIT);
        assert_ne!(first_roll, second_roll);
    }

    #[test]
    fn take_equip_and_use_update_structured_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        let take_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "medkit".to_owned(),
            },
        );
        assert!(state.inventory.iter().any(|item| item.id == "medkit"));
        assert!(
            state.location_items["kitchen"]
                .iter()
                .all(|item_id| item_id != "medkit")
        );
        assert!(matches!(
            take_outcome.events.first(),
            Some(GameEvent::ItemTaken { item_id }) if item_id == "medkit"
        ));
        assert!(
            !take_outcome
                .lines
                .iter()
                .any(|line| line.starts_with("Objective progress:"))
        );

        let equip_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "cricket_bat".to_owned(),
            },
        );
        assert_eq!(state.equipped_item_id.as_deref(), Some("cricket_bat"));
        assert!(matches!(
            equip_outcome.events.first(),
            Some(GameEvent::ItemEquipped { item_id }) if item_id == "cricket_bat"
        ));

        state.hp = 6;
        let use_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "medkit".to_owned(),
            },
        );
        assert_eq!(state.hp, 10);
        assert!(!state.inventory.iter().any(|item| item.id == "medkit"));
        assert!(matches!(
            use_outcome.events.first(),
            Some(GameEvent::ItemUsed {
                item_id,
                effect: ItemUseEffect::Healing { amount: 4 }
            }) if item_id == "medkit"
        ));
    }

    #[test]
    fn torch_reveals_connected_locations_and_then_stabilizes() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        assert_eq!(state.known_locations.len(), 1);

        let first_use = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "torch".to_owned(),
            },
        );

        assert!(state.known_locations.contains("kitchen"));
        assert!(state.known_locations.contains("garage"));
        assert!(state.known_locations.contains("back_garden"));
        assert!(matches!(
            first_use.events.first(),
            Some(GameEvent::ItemUsed {
                item_id,
                effect: ItemUseEffect::RevealedLocations { count: 3 }
            }) if item_id == "torch"
        ));
        assert!(first_use.lines.iter().any(|line| {
            line == "You sweep the torch across the exits and get a better read on the nearby routes."
        }));

        let second_use = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "torch".to_owned(),
            },
        );

        assert!(matches!(
            second_use.events.first(),
            Some(GameEvent::ItemUsed {
                item_id,
                effect: ItemUseEffect::RevealedLocations { count: 0 }
            }) if item_id == "torch"
        ));
        assert_eq!(
            second_use.lines.first().map(String::as_str),
            Some("The torch does not reveal anything new from here.")
        );
    }

    #[test]
    fn locked_garage_requires_house_keys_and_unlocks_cleanly() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let blocked_move = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "front_verandah");
        assert!(state.locked_locations.contains("garage"));
        assert_eq!(
            blocked_move.lines.first().map(String::as_str),
            Some("The garage door is locked. You need the house keys.")
        );
        let blocked_back_garden = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        assert_eq!(
            blocked_back_garden.lines.first().map(String::as_str),
            Some("The back gate is chained shut. You need the house keys.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );

        let wrong_context = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "house_keys".to_owned(),
            },
        );
        assert_eq!(
            wrong_context.lines.first().map(String::as_str),
            Some("That unlock item does not help here.")
        );
        assert!(state.locked_locations.contains("garage"));

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );

        let ambiguous_unlock = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "house_keys".to_owned(),
            },
        );
        assert_eq!(
            ambiguous_unlock.lines.first().map(String::as_str),
            Some(
                "More than one gate matches this item here: Garage, Back Garden. Use unlock <location>."
            )
        );
        assert!(state.locked_locations.contains("garage"));
        assert!(state.locked_locations.contains("back_garden"));

        let unlock_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        assert!(!state.locked_locations.contains("garage"));
        assert!(unlock_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationUnlocked {
                location_id,
                item_id
            } if location_id == "garage" && item_id == "house_keys"
        )));
        assert_eq!(
            unlock_outcome.lines.first().map(String::as_str),
            Some("You unlock Garage with house keys.")
        );

        let moved = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "garage");
        assert!(matches!(
            moved.events.first(),
            Some(GameEvent::Moved { to_location_id, .. }) if to_location_id == "garage"
        ));
    }

    #[test]
    fn unlock_command_alias_unlocks_reachable_gate() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );

        let unlock_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );

        assert!(!state.locked_locations.contains("garage"));
        assert!(unlock_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationUnlocked { location_id, .. } if location_id == "garage"
        )));
    }

    #[test]
    fn explicit_unlock_command_can_target_second_gate() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );

        let unlock_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );

        assert!(!state.locked_locations.contains("back_garden"));
        assert!(state.locked_locations.contains("garage"));
        assert!(unlock_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationUnlocked { location_id, .. } if location_id == "back_garden"
        )));
    }

    #[test]
    fn inspect_location_surfaces_gate_state_and_item_unlock_targets() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let garage_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "garage".to_owned(),
            },
        );
        assert!(
            garage_inspect
                .lines
                .iter()
                .any(|line| line.contains("Gate state: Locked"))
        );
        assert!(
            garage_inspect
                .lines
                .iter()
                .any(|line| line.contains("unlock item: house_keys"))
        );
        let back_garden_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "back_garden".to_owned(),
            },
        );
        assert!(
            back_garden_inspect
                .lines
                .iter()
                .any(|line| line.contains("Gate state: Locked"))
        );
        let front_verandah_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "front_verandah".to_owned(),
            },
        );
        assert!(
            front_verandah_inspect
                .lines
                .iter()
                .any(|line| { line.contains("Barricade state: Unbarricaded") })
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        let key_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "house_keys".to_owned(),
            },
        );
        assert!(
            key_inspect
                .lines
                .iter()
                .any(|line| line.contains("Can unlock: Garage, Back Garden."))
        );
    }

    #[test]
    fn inspect_enemy_surfaces_live_threat_state_and_route_read() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "back_garden".to_owned();
        state.locked_locations.remove("back_garden");
        state.known_locations.insert("back_garden".to_owned());
        state.visited_locations.insert("back_garden".to_owned());

        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "crawler_in_weeds".to_owned(),
            },
        );

        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Threat state: active | HP remaining: 3")
        );
        assert!(inspect.lines.iter().any(|line| line == "Present here: yes"));
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Senses: hearing yes | sight yes")
        );
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| { line.contains("this is the flank tax") })
        );
    }

    #[test]
    fn inspect_boss_surfaces_wounded_phase_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        state.locked_locations.remove("garage");
        state.known_locations.insert("garage".to_owned());
        state.visited_locations.insert("garage".to_owned());
        state.boss_hp.insert("brute_in_garage".to_owned(), 4);

        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "brute_in_garage".to_owned(),
            },
        );

        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Threat state: active | HP remaining: 4")
        );
        assert!(inspect.lines.iter().any(|line| line == "Present here: yes"));
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Senses: hearing yes | sight yes")
        );
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| { line.contains("Final phase: wounded and swinging harder") })
        );
    }

    #[test]
    fn look_surfaces_context_for_noisy_front_threshold() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.lines.iter().any(|line| {
            line.contains(
                "every loose sound on the property seems to funnel back toward the front step",
            )
        }));
    }

    #[test]
    fn look_surfaces_context_for_cleared_front_threshold() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.enemies_alive.remove("shambler_front_gate");

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.lines.iter().any(|line| {
            line.contains(
                "the front threshold is still a mess, but at least it belongs to you again",
            )
        }));
    }

    #[test]
    fn crawler_retaliation_surfaces_lane_specific_combat_text() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "back_garden".to_owned();
        state.locked_locations.remove("back_garden");
        state.known_locations.insert("back_garden".to_owned());
        state.visited_locations.insert("back_garden".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(
            outcome
                .lines
                .iter()
                .any(|line| { line.contains("It comes in low and hateful") })
        );
    }

    #[test]
    fn look_surfaces_context_for_secured_back_garden() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.lines.iter().any(|line| {
            line.contains("this becomes a rare place to recover 2 HP and regroup")
        }));
    }

    #[test]
    fn waiting_at_front_verandah_hurts_until_it_is_barricaded() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let first_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 9);
        assert_eq!(state.noise_level, 0);
        assert!(first_wait.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken {
                amount: 1,
                remaining_hp: 9
            }
        )));

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );

        let wrong_place = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );
        assert_eq!(
            wrong_place.lines.first().map(String::as_str),
            Some("You need to be there to barricade it.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        let barricade = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );

        assert!(state.barricaded_locations.contains("front_verandah"));
        assert!(barricade.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationBarricaded {
                location_id,
                item_id
            } if location_id == "front_verandah" && item_id == "barricade_kit"
        )));

        let hp_before_safe_wait = state.hp;
        let safe_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, hp_before_safe_wait);
        assert!(safe_wait.lines.iter().any(|line| {
            line.contains("The barricade takes the edge off the front gate pressure")
        }));
        assert!(
            !safe_wait
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { .. }))
        );
    }

    #[test]
    fn front_verandah_barricade_blocks_shambler_retaliation() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );

        let hp_before_attack = state.hp;
        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.hp, hp_before_attack);
        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                target_id,
                target_kind: EncounterKind::Enemy,
                defeated: false,
                ..
            } if target_id == "shambler_front_gate"
        )));
        assert!(
            !attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { .. }))
        );
        assert!(attack.lines.iter().any(|line| {
            line.contains("The barricade keeps the threat at splinter-spitting distance")
        }));
    }

    #[test]
    fn front_verandah_barricade_adds_attack_bonus_against_the_threshold_threat() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(matches!(
            attack.events.first(),
            Some(GameEvent::AttackResolved {
                target_kind: EncounterKind::Enemy,
                damage: 2,
                defeated: false,
                ..
            })
        ));
        assert!(
            attack
                .lines
                .iter()
                .any(|line| { line.contains("Attack bonus: +1") })
        );
    }

    #[test]
    fn waiting_in_back_garden_hurts_until_it_is_barricaded() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );

        let first_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert!(
            first_wait
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 1, .. }))
        );
        let hp_after_wait = state.hp;

        let barricade = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );
        assert!(state.barricaded_locations.contains("back_garden"));
        assert!(barricade.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationBarricaded {
                location_id,
                item_id
            } if location_id == "back_garden" && item_id == "barricade_kit"
        )));
        assert_eq!(state.hp, (hp_after_wait + 2).min(state.max_hp));
        assert!(
            barricade
                .lines
                .iter()
                .any(|line| { line.contains("HP rises from") })
        );

        let hp_before_safe_wait = state.hp;
        let safe_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, hp_before_safe_wait);
        assert!(safe_wait.lines.iter().any(|line| {
            line.contains("The back barricade keeps the weeds from becoming a bite problem")
        }));
        assert!(
            !safe_wait
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { .. }))
        );
    }

    #[test]
    fn loud_actions_raise_noise_and_successful_wait_lowers_it() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert_eq!(state.noise_level, 1);
        assert!(
            attack
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        let unlock = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        assert_eq!(state.noise_level, 1);
        assert!(
            unlock
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        let barricade = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );
        assert_eq!(state.noise_level, 1);
        assert!(
            barricade
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );

        let calm_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.noise_level, 0);
        assert!(
            calm_wait
                .lines
                .iter()
                .any(|line| line == "Noise settles to Quiet.")
        );
    }

    #[test]
    fn successful_non_noisy_actions_reduce_noise_over_time() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let look = apply_action(&mut state, &bundle, GameAction::Look);

        assert_eq!(state.noise_level, 1);
        assert!(
            look.lines
                .iter()
                .any(|line| line == "Noise settles to Stirred.")
        );

        let move_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert_eq!(state.noise_level, 0);
        assert!(
            move_outcome
                .lines
                .iter()
                .any(|line| line == "Noise settles to Quiet.")
        );
    }

    #[test]
    fn rejected_non_noisy_actions_do_not_reduce_noise() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let blocked = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        assert_eq!(state.noise_level, 2);
        assert!(
            blocked
                .events
                .iter()
                .any(|event| { matches!(event, GameEvent::MovementBlocked { .. }) })
        );
        assert!(
            !blocked
                .lines
                .iter()
                .any(|line| line.starts_with("Noise settles"))
        );
    }

    #[test]
    fn max_noise_spawns_enemy_instance_in_yard_location() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        let Some(GameEvent::NoiseSpawnedEnemy {
            enemy_id,
            template_id,
            location_id,
        }) = outcome
            .events
            .iter()
            .find(|event| matches!(event, GameEvent::NoiseSpawnedEnemy { .. }))
        else {
            panic!("expected max-noise spawn event");
        };
        let spawn_location = bundle
            .locations
            .iter()
            .find(|location| location.id == *location_id)
            .expect("spawned location should resolve");
        let spawned_template = bundle
            .enemies
            .iter()
            .find(|enemy| enemy.id == *template_id)
            .expect("spawned enemy template should resolve");

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 1);
        assert!(enemy_id.starts_with("noise_spawn_1_"));
        assert!(spawn_location.tags.iter().any(|tag| tag == "outdoor"));
        assert!(state.enemies_alive.contains(enemy_id));
        assert_eq!(state.enemy_hp.get(enemy_id), Some(&spawned_template.hp));
        assert!(
            state
                .location_enemies
                .get(location_id)
                .is_some_and(|entries| entries.contains(enemy_id))
        );
        assert!(outcome.lines.iter().any(|line| {
            line.contains("Noise peaks at Swarming") && line.contains(&spawn_location.name)
        }));
        assert!(!outcome.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::SpawnedEnemyMoved { enemy_id: moved_id, .. }
                    | GameEvent::SpawnedEnemyWaited {
                        enemy_id: moved_id,
                        ..
                    } if moved_id == enemy_id
            )
        }));

        let spawned_enemy_id = enemy_id.clone();
        let spawned_template_id = template_id.clone();
        let spawned_location_id = location_id.clone();
        state.current_location_id = spawned_location_id;
        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: spawned_template_id,
            },
        );
        assert!(matches!(
            inspect.events.first(),
            Some(GameEvent::Inspected { target }) if target == &spawned_enemy_id
        ));
    }

    #[test]
    fn noise_spawn_only_fires_when_noise_crosses_into_maximum() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 3;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 0);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| { matches!(event, GameEvent::NoiseSpawnedEnemy { .. }) })
        );
    }

    #[test]
    fn max_noise_does_not_spawn_enemy_when_pool_cannot_hear() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        for enemy in &mut bundle.enemies {
            enemy.can_hear = false;
        }
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 0);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| { matches!(event, GameEvent::NoiseSpawnedEnemy { .. }) })
        );
    }

    #[test]
    fn existing_spawned_enemy_moves_one_legal_tile_toward_noise_source() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "laundry".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(
            !state
                .location_enemies
                .get("back_garden")
                .is_some_and(|entries| entries.contains(&enemy_id))
        );
        assert!(
            state
                .location_enemies
                .get("front_verandah")
                .is_some_and(|entries| entries.contains(&enemy_id))
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyMoved {
                enemy_id: moved_id,
                from_location_id,
                to_location_id,
                target_location_id,
            } if moved_id == &enemy_id
                && from_location_id == "back_garden"
                && to_location_id == "front_verandah"
                && target_location_id == "front_verandah"
        )));
    }

    #[test]
    fn noisy_actions_shift_existing_spawned_enemy_attractor_to_latest_source() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "kitchen".to_owned();
        state.noise_level = 3;
        state.noise_spawn_count = 1;
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        state.spawned_enemy_searching.insert(enemy_id.clone());

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(
            state
                .spawned_enemy_targets
                .get(&enemy_id)
                .map(String::as_str),
            Some("kitchen")
        );
        assert!(!state.spawned_enemy_searching.contains(&enemy_id));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::NoiseAttractorShifted {
                location_id,
                enemy_ids,
            } if location_id == "kitchen" && enemy_ids == &vec![enemy_id.clone()]
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyMoved {
                enemy_id: moved_id,
                from_location_id,
                to_location_id,
                target_location_id,
            } if moved_id == &enemy_id
                && from_location_id == "back_garden"
                && to_location_id == "front_verandah"
                && target_location_id == "kitchen"
        )));
    }

    #[test]
    fn spawned_enemy_searches_after_reaching_noise_source() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "laundry".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("front_verandah".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        state
            .spawned_enemy_origins
            .insert(enemy_id.clone(), "kitchen".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(state.spawned_enemy_searching.contains(&enemy_id));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyWaited {
                enemy_id: waited_id,
                reason,
                ..
            } if waited_id == &enemy_id && reason.contains("searching")
        ) || matches!(
            event,
            GameEvent::SpawnedEnemyMoved {
                enemy_id: moved_id,
                from_location_id,
                ..
            } if moved_id == &enemy_id && from_location_id == "front_verandah"
        )));
    }

    #[test]
    fn spawned_enemy_acquires_player_by_sight_and_prioritizes_visual_chase() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut acquired = false;

        for turn_index in 0..40 {
            let mut state = generate_new_run(&bundle).state;
            state.current_location_id = "front_verandah".to_owned();
            state.turn_index = turn_index;
            state.enemies_alive.insert(enemy_id.clone());
            state.enemy_hp.insert(enemy_id.clone(), 3);
            state
                .location_enemies
                .entry("kitchen".to_owned())
                .or_default()
                .push(enemy_id.clone());
            state
                .spawned_enemy_targets
                .insert(enemy_id.clone(), "laundry".to_owned());
            state
                .spawned_enemy_origins
                .insert(enemy_id.clone(), "back_garden".to_owned());

            let outcome = apply_action(&mut state, &bundle, GameAction::Look);
            if !outcome.events.iter().any(|event| {
                matches!(event, GameEvent::SightAttractorAcquired { enemy_id: sighting_id, .. } if sighting_id == &enemy_id)
            }) {
                continue;
            }

            assert_eq!(
                state
                    .spawned_enemy_sight_targets
                    .get(&enemy_id)
                    .map(String::as_str),
                Some("front_verandah")
            );
            assert_eq!(
                state
                    .spawned_enemy_sight_subjects
                    .get(&enemy_id)
                    .map(String::as_str),
                Some("player")
            );
            assert!(outcome.events.iter().any(|event| matches!(
                event,
                GameEvent::SightAttractorAcquired {
                    enemy_id: sighting_id,
                    subject_id,
                    location_id,
                } if sighting_id == &enemy_id
                    && subject_id == "player"
                    && location_id == "front_verandah"
            )));
            assert!(outcome.events.iter().any(|event| matches!(
                event,
                GameEvent::SpawnedEnemyMoved {
                    enemy_id: moved_id,
                    target_location_id,
                    ..
                } if moved_id == &enemy_id && target_location_id == "front_verandah"
            ) || matches!(
                event,
                GameEvent::SpawnedEnemyWaited {
                    enemy_id: waited_id,
                    reason,
                    ..
                } if waited_id == &enemy_id && reason.contains("chasing player by sight")
            )));
            acquired = true;
            break;
        }

        assert!(
            acquired,
            "expected at least one deterministic sight acquisition in searched states"
        );
    }

    #[test]
    fn rules_can_disable_sight_acquisition_chance() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!state.spawned_enemy_sight_targets.contains_key(&enemy_id));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorMissed {
                enemy_id: missed_id,
                detect_chance_percent: 0,
                ..
            } if missed_id == &enemy_id
        )));
    }

    #[test]
    fn rules_can_disable_sight_chase_delay() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        bundle.rules.sight_chase_delay_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyMoved {
                enemy_id: moved_id,
                target_location_id,
                ..
            } if moved_id == &enemy_id && target_location_id == "front_verandah"
        )));
        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyWaited {
                enemy_id: waited_id,
                reason,
                ..
            } if waited_id == &enemy_id && reason.contains("chasing player by sight")
        )));
    }

    #[test]
    fn player_sight_overrides_active_noise_attractor() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        bundle.rules.sight_chase_delay_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), "laundry".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired {
                enemy_id: sighting_id,
                subject_id,
                location_id,
            } if sighting_id == &enemy_id
                && subject_id == "player"
                && location_id == "front_verandah"
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyMoved {
                enemy_id: moved_id,
                to_location_id,
                target_location_id,
                ..
            } if moved_id == &enemy_id
                && to_location_id == "front_verandah"
                && target_location_id == "front_verandah"
        )));
    }

    #[test]
    fn non_player_sight_does_not_override_active_noise_attractor() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        state.location_enemies.clear();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .location_enemies
            .entry("laundry".to_owned())
            .or_default()
            .push("crawler_in_weeds".to_owned());
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired {
                enemy_id: sighting_id,
                subject_id,
                ..
            } if sighting_id == &enemy_id && subject_id == "crawler_in_weeds"
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyMoved {
                enemy_id: moved_id,
                target_location_id,
                ..
            } if moved_id == &enemy_id && target_location_id == "front_verandah"
        )));
    }

    #[test]
    fn spawned_enemy_cannot_acquire_sight_when_template_cannot_see() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let shambler = bundle
            .enemies
            .iter_mut()
            .find(|enemy| enemy.id == "shambler_front_gate")
            .expect("expected shambler template");
        shambler.can_see = false;
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .spawned_enemy_origins
            .insert(enemy_id.clone(), "back_garden".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!state.spawned_enemy_sight_targets.contains_key(&enemy_id));
        assert!(!outcome.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::SightAttractorAcquired { enemy_id: sighting_id, .. }
                    | GameEvent::SightAttractorMissed { enemy_id: sighting_id, .. }
                    if sighting_id == &enemy_id
            )
        }));
    }

    #[test]
    fn spawned_enemy_sight_check_can_fail_before_acquisition() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut miss_outcome = None;

        for turn_index in 0..40 {
            let mut state = generate_new_run(&bundle).state;
            state.current_location_id = "front_verandah".to_owned();
            state.turn_index = turn_index;
            state.enemies_alive.insert(enemy_id.clone());
            state.enemy_hp.insert(enemy_id.clone(), 3);
            state
                .location_enemies
                .entry("kitchen".to_owned())
                .or_default()
                .push(enemy_id.clone());
            state
                .spawned_enemy_origins
                .insert(enemy_id.clone(), "back_garden".to_owned());

            let outcome = apply_action(&mut state, &bundle, GameAction::Look);
            if outcome.events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::SightAttractorMissed {
                        enemy_id: missed_id,
                        subject_id,
                        location_id,
                        detect_chance_percent,
                        ..
                    } if missed_id == &enemy_id
                        && subject_id == "player"
                        && location_id == "front_verandah"
                        && *detect_chance_percent == bundle.rules.sight_acquire_chance_percent
                )
            }) {
                assert!(!state.spawned_enemy_sight_targets.contains_key(&enemy_id));
                miss_outcome = Some(outcome);
                break;
            }
        }

        assert!(
            miss_outcome.is_some(),
            "expected at least one deterministic sight miss in searched states"
        );
    }

    #[test]
    fn delayed_sight_chase_can_be_shaken_into_search() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "garage".to_owned();
        state.location_enemies.clear();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .spawned_enemy_targets
            .insert(enemy_id.clone(), "laundry".to_owned());
        state
            .spawned_enemy_origins
            .insert(enemy_id.clone(), "back_garden".to_owned());
        state
            .spawned_enemy_sight_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        state
            .spawned_enemy_sight_subjects
            .insert(enemy_id.clone(), "player".to_owned());
        state.spawned_enemy_sight_delays.insert(enemy_id.clone(), 1);

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(state.spawned_enemy_searching.contains(&enemy_id));
        assert!(!state.spawned_enemy_sight_targets.contains_key(&enemy_id));
        assert!(!state.spawned_enemy_sight_subjects.contains_key(&enemy_id));
        assert!(!state.spawned_enemy_sight_delays.contains_key(&enemy_id));
        assert!(outcome.lines.iter().any(|line| line.contains("searching")));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorLost {
                enemy_id: lost_id,
                subject_id,
            } if lost_id == &enemy_id && subject_id == "player"
        )));
    }

    #[test]
    fn spawned_enemy_can_acquire_other_enemy_by_sight() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "garage".to_owned();
        state.location_enemies.clear();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .location_enemies
            .entry("laundry".to_owned())
            .or_default()
            .push("crawler_in_weeds".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert_eq!(
            state
                .spawned_enemy_sight_targets
                .get(&enemy_id)
                .map(String::as_str),
            Some("laundry")
        );
        assert_eq!(
            state
                .spawned_enemy_sight_subjects
                .get(&enemy_id)
                .map(String::as_str),
            Some("crawler_in_weeds")
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired {
                enemy_id: sighting_id,
                subject_id,
                location_id,
            } if sighting_id == &enemy_id
                && subject_id == "crawler_in_weeds"
                && location_id == "laundry"
        )));
    }

    #[test]
    fn searching_spawned_enemy_can_choose_to_return_to_origin() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.noise_spawn_count = 1;
        state
            .spawned_enemy_origins
            .insert(enemy_id.clone(), "kitchen".to_owned());

        let mut return_step = None;
        for turn_index in 0..20 {
            state.turn_index = turn_index;
            if let SpawnedEnemyStep::Move {
                to_location_id,
                step_target_location_id: Some(step_target_location_id),
            } = spawned_enemy_next_step(
                &state,
                &bundle,
                &enemy_id,
                "front_verandah",
                "front_verandah",
                true,
            ) {
                return_step = Some((to_location_id, step_target_location_id));
                break;
            }
        }

        assert_eq!(
            return_step,
            Some(("kitchen".to_owned(), "kitchen".to_owned()))
        );
    }

    #[test]
    fn spawned_enemy_attacks_barricade_hazard_and_can_fail_to_break_it() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let barricade_blocked_id = "noise_spawn_1_crawler_in_weeds".to_owned();
        state.enemies_alive.insert(barricade_blocked_id.clone());
        state.enemy_hp.insert(barricade_blocked_id.clone(), 2);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(barricade_blocked_id.clone());
        state
            .spawned_enemy_targets
            .insert(barricade_blocked_id.clone(), "front_verandah".to_owned());
        state
            .barricaded_locations
            .insert("front_verandah".to_owned());

        let barricade_outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(
            state
                .location_enemies
                .get("back_garden")
                .is_some_and(|entries| entries.contains(&barricade_blocked_id))
        );
        assert!(state.barricaded_locations.contains("front_verandah"));
        assert!(barricade_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyAttackedHazard {
                enemy_id,
                hazard_kind: MovementHazardKind::Barricade,
                location_id,
                broken: false,
                ..
            } if enemy_id == &barricade_blocked_id
                && location_id == "front_verandah"
        )));
    }

    #[test]
    fn rules_can_force_spawned_hazard_breaks() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.spawned_hazard_break_chance_percent = 100;
        let mut state = generate_new_run(&bundle).state;
        let barricade_blocked_id = "noise_spawn_1_crawler_in_weeds".to_owned();
        state.enemies_alive.insert(barricade_blocked_id.clone());
        state.enemy_hp.insert(barricade_blocked_id.clone(), 2);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(barricade_blocked_id.clone());
        state
            .spawned_enemy_targets
            .insert(barricade_blocked_id.clone(), "front_verandah".to_owned());
        state
            .barricaded_locations
            .insert("front_verandah".to_owned());

        let barricade_outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!state.barricaded_locations.contains("front_verandah"));
        assert!(barricade_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyAttackedHazard {
                enemy_id,
                hazard_kind: MovementHazardKind::Barricade,
                location_id,
                break_chance_percent: 100,
                broken: true,
                ..
            } if enemy_id == &barricade_blocked_id
                && location_id == "front_verandah"
        )));
    }

    #[test]
    fn spawned_enemy_can_break_locked_gate_into_broken_open_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let locked_blocked_id = "noise_spawn_2_shambler_front_gate".to_owned();
        state.current_location_id = "laundry".to_owned();
        state.noise_spawn_count = 1;
        state.enemies_alive.insert(locked_blocked_id.clone());
        state.enemy_hp.insert(locked_blocked_id.clone(), 3);
        state
            .location_enemies
            .entry("front_verandah".to_owned())
            .or_default()
            .push(locked_blocked_id.clone());
        state
            .spawned_enemy_targets
            .insert(locked_blocked_id.clone(), "garage".to_owned());

        let locked_outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(
            state
                .location_enemies
                .get("front_verandah")
                .is_some_and(|entries| entries.contains(&locked_blocked_id))
        );
        assert!(locked_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SpawnedEnemyAttackedHazard {
                enemy_id,
                hazard_kind: MovementHazardKind::LockedGate,
                location_id,
                broken: true,
                ..
            } if enemy_id == &locked_blocked_id
                && location_id == "garage"
        )));
        assert!(!state.locked_locations.contains("garage"));
        assert!(state.broken_locked_locations.contains("garage"));

        state.current_location_id = "front_verandah".to_owned();
        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "garage".to_owned(),
            },
        );
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line.contains("Gate state: Broken"))
        );

        let moved = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "garage");
        assert!(moved.events.iter().any(|event| {
            matches!(event, GameEvent::Moved { to_location_id, .. } if to_location_id == "garage")
        }));
    }

    #[test]
    fn exposed_pressure_and_retaliation_scale_up_at_high_noise() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 8);
        assert_eq!(state.noise_level, 1);
        assert!(wait.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken {
                amount: 2,
                remaining_hp: 8
            }
        )));

        state.noise_level = 2;
        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 3, .. }))
        );
    }

    #[test]
    fn boss_combat_completes_objective_and_surfaces_win() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "cricket_bat".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        assert!(!state.active_objective.completed);
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        let first = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(matches!(
            first.events.first(),
            Some(GameEvent::AttackResolved {
                target_kind: EncounterKind::Boss,
                damage: 3,
                defeated: false,
                ..
            })
        ));
        assert_eq!(state.hp, 7);
        assert!(state.bosses_alive.contains("brute_in_garage"));

        apply_action(&mut state, &bundle, GameAction::Attack);
        let final_outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(!state.bosses_alive.contains("brute_in_garage"));
        assert!(state.bosses_defeated.contains("brute_in_garage"));
        assert!(state.active_objective.completed);
        assert!(final_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                target_kind: EncounterKind::Boss,
                defeated: true,
                ..
            }
        )));
        assert!(
            final_outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
        assert!(
            final_outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::RunWon))
        );
        assert!(final_outcome.lines.iter().any(|line| {
            line == "Objective progress: Defeat boss 'brute_in_garage' is now complete."
        }));
        assert!(final_outcome.lines.iter().any(|line| line == "You win."));
    }

    #[test]
    fn garage_brute_wounded_phase_hits_harder_and_surfaces_phase_text() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        state.boss_hp.insert("brute_in_garage".to_owned(), 5);
        let hp_before = state.hp;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(attack.lines.iter().any(|line| {
            line.contains("becomes more dangerous once it realizes it should already be dead")
        }));
        assert!(attack.lines.iter().any(|line| {
            line.contains("Final-phase pressure: the brute is hitting harder now")
        }));
        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 4, .. }))
        );
        assert_eq!(state.hp, hp_before - 4);
    }

    #[test]
    fn secured_siege_lanes_reduce_garage_brute_retaliation() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "cricket_bat".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        state.boss_hp.insert("brute_in_garage".to_owned(), 5);
        let hp_before = state.hp;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 3, .. }))
        );
        assert_eq!(state.hp, hp_before - 3);
        assert!(
            attack
                .lines
                .iter()
                .any(|line| { line.contains("Retaliation reduced by 1") })
        );
        assert!(
            attack
                .lines
                .iter()
                .any(|line| { line.contains("Final-phase pressure") })
        );
    }

    #[test]
    fn item_only_objective_completes_when_required_item_is_taken() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.target_boss_id = None;
        state.active_objective.required_item_id = Some("house_keys".to_owned());
        state.active_objective.required_location_id = None;
        state.active_objective.completed = false;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );

        assert!(state.active_objective.completed);
        assert!(
            outcome.lines.iter().any(|line| {
                line == "Objective progress: Hold item 'house_keys' is now complete."
            })
        );
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
        assert!(outcome.lines.iter().any(|line| line == "You win."));
    }

    #[test]
    fn location_only_objective_completes_when_required_location_is_reached() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.target_boss_id = None;
        state.active_objective.required_item_id = None;
        state.active_objective.required_location_id = Some("kitchen".to_owned());
        state.active_objective.completed = false;

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert!(state.active_objective.completed);
        assert!(outcome.lines.iter().any(|line| {
            line == "Objective progress: Reach location 'kitchen' is now complete."
        }));
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
    }

    #[test]
    fn combined_objective_requires_location_in_addition_to_other_conditions() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );

        state.active_objective.target_boss_id = None;
        state.active_objective.required_item_id = Some("house_keys".to_owned());
        state.active_objective.required_location_id = Some("garage".to_owned());
        state.active_objective.completed = false;

        let before_entering = apply_action(&mut state, &bundle, GameAction::Look);
        assert!(!state.active_objective.completed);
        assert!(!before_entering.lines.iter().any(|line| line == "You win."));

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        assert!(state.active_objective.completed);
        assert!(outcome.lines.iter().any(|line| {
            line == "Objective progress: Reach location 'garage' is now complete."
        }));
        assert!(outcome.lines.iter().any(|line| line == "You win."));
    }

    #[test]
    fn epilogue_mode_blocks_combat_and_pressure_without_blocking_exploration() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.completed = true;
        state.current_location_id = "garage".to_owned();
        state.locked_locations.remove("garage");
        state.noise_level = 3;
        state.hp = 5;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert_eq!(state.hp, 5);
        assert_eq!(state.noise_level, 3);
        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::ActionRejected { reason } if reason.contains("already won")
        )));

        let wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 5);
        assert_eq!(state.noise_level, 3);
        assert!(
            wait.lines
                .iter()
                .any(|line| line.contains("siege clock is no longer spending your HP"))
        );

        let look = apply_action(&mut state, &bundle, GameAction::Look);
        assert!(look.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationLooked { location_id } if location_id == "garage"
        )));
        assert!(
            look.lines
                .iter()
                .any(|line| line.contains("finally gives up being an arena"))
        );
        assert!(
            look.lines
                .iter()
                .any(|line| line.contains("Aftermath hook: Post-credits hook"))
        );

        let move_out = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "front_verandah");
        assert!(move_out.events.iter().any(|event| matches!(
            event,
            GameEvent::Moved {
                to_location_id, ..
            } if to_location_id == "front_verandah"
        )));
    }

    #[test]
    fn rolling_summary_is_bounded_and_event_confirmed() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        for _ in 0..(ROLLING_SUMMARY_LIMIT + 8) {
            apply_action(&mut state, &bundle, GameAction::Look);
        }

        assert_eq!(state.rolling_summary.len(), ROLLING_SUMMARY_LIMIT);
        assert!(
            state
                .rolling_summary
                .iter()
                .all(|line| line == "Looked around location 'front_verandah'.")
        );
    }

    #[test]
    fn rolling_summary_records_epilogue_actions_without_reopening_danger() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.completed = true;
        state.hp = 5;
        state.noise_level = 3;

        apply_action(&mut state, &bundle, GameAction::Wait);

        assert_eq!(state.hp, 5);
        assert_eq!(state.noise_level, 3);
        assert!(
            state
                .rolling_summary
                .iter()
                .any(|line| { line.contains("Action rejected: The run is already won") })
        );
    }
}
