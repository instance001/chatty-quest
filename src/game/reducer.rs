use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, EncounterKind, GameAction, GameEvent, ItemUseEffect};
use super::queries::{
    describe_current_location, describe_location, equipped_damage, find_boss,
    find_boss_by_name_or_id, find_enemy, find_enemy_by_name_or_id, find_item,
    find_item_by_name_or_id, find_location, find_location_by_name_or_id, is_location_barricaded,
    is_location_locked, matches_name, unlock_targets_for_item,
};
use super::state::{InventoryEntry, RunState};

const ROLLING_SUMMARY_LIMIT: usize = 24;

pub fn apply_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: GameAction,
) -> ActionOutcome {
    if state.active_objective.completed {
        let outcome = apply_epilogue_action(state, bundle, action);
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

    apply_noise_for_action(state, &action_for_noise, &mut outcome);

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
    lines.extend(movement_context_lines(state, destination_location));

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
        lines.extend(item_context_lines(state, &item.id));
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: item.id.clone(),
            }],
            lines,
        };
    }

    if let Some(enemy) = find_enemy_by_name_or_id(bundle, target) {
        let mut lines = vec![format!("{}: {}", enemy.name, enemy.description)];
        lines.extend(inspect_enemy_state_lines(state, &enemy.id));
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: enemy.id.clone(),
            }],
            lines,
        };
    }

    if let Some(boss) = find_boss_by_name_or_id(bundle, target) {
        let mut lines = vec![format!("{}: {}", boss.name, boss.description)];
        lines.extend(inspect_boss_state_lines(state, &boss.id));
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
            lines.extend(item_pickup_lines(&item.id));
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

    let mut lines = if newly_known.is_empty() {
        vec!["The torch does not reveal anything new from here.".to_owned()]
    } else {
        vec![
            "You sweep the torch across the exits and get a better read on the nearby routes."
                .to_owned(),
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
            if let Some(entries) = state.location_enemies.get_mut(&current_location) {
                entries.retain(|entry| entry != &enemy_id);
            }
            let enemy_name = find_enemy(bundle, &enemy_id)
                .map(|enemy| enemy.name.clone())
                .unwrap_or_else(|| enemy_id.clone());
            lines.push(format!("{} goes down.", enemy_name));
            if let Some(defeat_line) = enemy_defeat_line(&enemy_id) {
                lines.push(defeat_line.to_owned());
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
                if let Some(retaliation_line) = enemy_retaliation_line(&enemy_id) {
                    lines.push(retaliation_line.to_owned());
                }
                lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
            }
        }

        return ActionOutcome { events, lines };
    }

    if let Some(boss_id) = boss_here {
        let player_damage = equipped_damage(state).max(1);
        let boss_damage = state.boss_hp.entry(boss_id.clone()).or_insert(0);
        *boss_damage -= player_damage;
        let boss_remaining_hp = *boss_damage;
        let wounded_phase = is_garage_brute_wounded_phase(&boss_id, boss_remaining_hp);

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
            lines.push(format!(
                "{} collapses. The worst thing on the block is finished.",
                boss_name
            ));
            if state.current_location_id == "garage"
                && state.active_objective.required_location_id.as_deref() == Some("garage")
            {
                lines.push(
                    "The garage finally sounds like a room again instead of a threat waiting to happen."
                        .to_owned(),
                );
            }
        } else {
            if wounded_phase {
                lines.push(
                    "The brute stumbles, resets, and somehow becomes more dangerous once it realizes it should already be dead."
                        .to_owned(),
                );
            }
            let secured_property_bonus =
                if boss_id == "brute_in_garage" && property_siege_lanes_secured(state) {
                    1
                } else {
                    0
                };
            let retaliation = (find_boss(bundle, &boss_id)
                .map(|boss| boss.damage)
                .unwrap_or(2)
                + if wounded_phase { 1 } else { 0 }
                - secured_property_bonus)
                .max(1);
            state.hp = (state.hp - retaliation).max(0);
            events.push(GameEvent::DamageTaken {
                amount: retaliation,
                remaining_hp: state.hp,
            });
            lines.push(format!("The boss smashes back for {} damage.", retaliation));
            if secured_property_bonus > 0 {
                lines.push(
                    "With both exposed approaches barricaded, the garage fight stops feeling like the whole property is joining in. Retaliation reduced by 1."
                        .to_owned(),
                );
            }
            if state.current_location_id == "garage" {
                lines.push(
                    "There is not enough space in the garage for both of you to make mistakes."
                        .to_owned(),
                );
            }
            if wounded_phase {
                lines.push(
                    "Final-phase pressure: the brute is hitting harder now that the room smells like an ending."
                        .to_owned(),
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
    let location_name = find_location(bundle, &state.current_location_id)
        .map(|location| location.name.clone())
        .unwrap_or_else(|| state.current_location_id.clone());

    let mut events = vec![GameEvent::Waited {
        location_id: state.current_location_id.clone(),
    }];
    let mut lines = vec![format!(
        "You wait at {} and listen to the property complain around you.",
        location_name
    )];

    if state.current_location_id == "front_verandah"
        && state.enemies_alive.contains("shambler_front_gate")
    {
        if is_location_barricaded(state, "front_verandah") {
            lines.push(
                "The barricade takes the edge off the front gate pressure. The shambler stays outside and ugly."
                    .to_owned(),
            );
        } else {
            let pressure = exposed_noise_pressure_damage(state);
            state.hp = (state.hp - pressure).max(0);
            events.push(GameEvent::DamageTaken {
                amount: pressure,
                remaining_hp: state.hp,
            });
            lines.push(
                "The Front Gate Shambler keeps scraping at the threshold until it costs you blood and patience."
                    .to_owned(),
            );
            if pressure > 1 {
                lines.push("The extra noise makes the front approach even uglier.".to_owned());
            }
            lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
        }
    }

    if state.current_location_id == "back_garden"
        && state.enemies_alive.contains("crawler_in_weeds")
    {
        if is_location_barricaded(state, "back_garden") {
            lines.push(
                "The back barricade keeps the weeds from becoming a bite problem for one blessed minute."
                    .to_owned(),
            );
        } else {
            let pressure = exposed_noise_pressure_damage(state);
            state.hp = (state.hp - pressure).max(0);
            events.push(GameEvent::DamageTaken {
                amount: pressure,
                remaining_hp: state.hp,
            });
            lines.push(
                "Something low and eager keeps testing the weeds until your ankles pay the tax."
                    .to_owned(),
            );
            if pressure > 1 {
                lines.push(
                    "The extra noise turns the flank into a worse idea by the second.".to_owned(),
                );
            }
            lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
        }
    }

    ActionOutcome { events, lines }
}

fn apply_noise_for_action(state: &mut RunState, action: &GameAction, outcome: &mut ActionOutcome) {
    match action {
        GameAction::Attack => raise_noise(state, 1, &mut outcome.lines),
        GameAction::Unlock { .. } => {
            if outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LocationUnlocked { .. }))
            {
                raise_noise(state, 1, &mut outcome.lines);
            }
        }
        GameAction::Barricade { .. } => {
            if outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LocationBarricaded { .. }))
            {
                raise_noise(state, 1, &mut outcome.lines);
            }
        }
        GameAction::Wait => {
            if state
                .barricaded_locations
                .contains(&state.current_location_id)
            {
                lower_noise(state, 1, &mut outcome.lines);
            }
        }
        _ => {}
    }
}

fn raise_noise(state: &mut RunState, amount: i32, lines: &mut Vec<String>) {
    let before = state.noise_level;
    state.noise_level = (state.noise_level + amount).clamp(0, 3);
    if state.noise_level != before {
        lines.push(format!(
            "Noise rises to {}.",
            noise_label(state.noise_level)
        ));
    }
}

fn lower_noise(state: &mut RunState, amount: i32, lines: &mut Vec<String>) {
    let before = state.noise_level;
    state.noise_level = (state.noise_level - amount).clamp(0, 3);
    if state.noise_level != before {
        lines.push(format!(
            "Noise settles to {}.",
            noise_label(state.noise_level)
        ));
    }
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
        && (location.id == "front_verandah" || location.id == "back_garden")
    {
        1
    } else {
        0
    }
}

fn is_garage_brute_wounded_phase(boss_id: &str, remaining_hp: i32) -> bool {
    boss_id == "brute_in_garage" && remaining_hp > 0 && remaining_hp <= 4
}

fn property_siege_lanes_secured(state: &RunState) -> bool {
    state.barricaded_locations.contains("front_verandah")
        && state.barricaded_locations.contains("back_garden")
}

fn enemy_retaliation_line(enemy_id: &str) -> Option<&'static str> {
    match enemy_id {
        "shambler_front_gate" => Some(
            "It keeps leaning its full dead weight into the threshold like the house personally offended it.",
        ),
        "crawler_in_weeds" => {
            Some("It comes in low and hateful, exactly where your attention keeps failing first.")
        }
        _ => None,
    }
}

fn enemy_defeat_line(enemy_id: &str) -> Option<&'static str> {
    match enemy_id {
        "shambler_front_gate" => {
            Some("The front step stops feeling argued with for the first time all night.")
        }
        "crawler_in_weeds" => Some("The weeds go back to being weeds, which is somehow a relief."),
        _ => None,
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
    location: &crate::data::datapacks::LocationTemplate,
) -> Vec<String> {
    let mut lines = Vec::new();

    if location.id == "garage" {
        let boss_alive = state
            .location_bosses
            .get("garage")
            .into_iter()
            .flatten()
            .any(|boss_id| state.bosses_alive.contains(boss_id));

        if boss_alive {
            lines.push(
                "Objective pressure: the house keys got you this far; the brute is the part that still has to be earned."
                    .to_owned(),
            );
            if state.barricaded_locations.contains("front_verandah") {
                lines.push(
                    "The secured front threshold leaves the fight behind you cleaner than the one ahead."
                        .to_owned(),
                );
            }
        }
    }

    lines
}

fn item_context_lines(state: &RunState, item_id: &str) -> Vec<String> {
    match item_id {
        "house_keys" => vec![
            "Use case: opens the garage and the chained back gate.".to_owned(),
            if state.active_objective.required_item_id.as_deref() == Some("house_keys") {
                "Objective pressure: you do not just need these once, you need to still be holding them at the end.".to_owned()
            } else {
                "Ordinary keys, extremely non-ordinary night.".to_owned()
            },
        ],
        "medkit" => vec![
            "Use case: restores 4 HP, then it is gone.".to_owned(),
            "This is the run's cleanest recovery spike, so wasting it usually hurts twice."
                .to_owned(),
        ],
        "barricade_kit" => vec![
            "Use case: secures a barricadable route when used from that location.".to_owned(),
            "In this scenario, it turns exposed space into breathing room instead of raw safety."
                .to_owned(),
        ],
        _ => Vec::new(),
    }
}

fn item_pickup_lines(item_id: &str) -> Vec<String> {
    match item_id {
        "house_keys" => {
            vec!["They feel much heavier now that the whole route depends on them.".to_owned()]
        }
        "medkit" => vec!["A tiny, crinkling argument against dying stupidly.".to_owned()],
        "barricade_kit" => {
            vec!["Not elegant materials, but they are the right kind of ugly.".to_owned()]
        }
        _ => Vec::new(),
    }
}

fn inspect_enemy_state_lines(state: &RunState, enemy_id: &str) -> Vec<String> {
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

    match enemy_id {
        "shambler_front_gate" if alive => lines.push(
            "Route read: as long as it stands, the front threshold remains the loud direct-pressure lane."
                .to_owned(),
        ),
        "shambler_front_gate" => lines.push(
            "Route read: the front threshold is mechanically calmer now that the shambler is down."
                .to_owned(),
        ),
        "crawler_in_weeds" if alive => lines.push(
            "Route read: this is the flank tax; leave it alive and the back edge stays mean."
                .to_owned(),
        ),
        "crawler_in_weeds" => lines.push(
            "Route read: with the crawler gone, the back route is no longer being actively contested."
                .to_owned(),
        ),
        _ => {}
    }

    lines
}

fn inspect_boss_state_lines(state: &RunState, boss_id: &str) -> Vec<String> {
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

    if is_garage_brute_wounded_phase(boss_id, remaining_hp) {
        lines
            .push("Final phase: wounded and swinging harder than the opening exchange.".to_owned());
    } else if boss_id == "brute_in_garage" && alive {
        lines.push(
            "Final phase: not yet. Once this thing is bloodied, the garage gets nastier in a hurry."
                .to_owned(),
        );
    } else if boss_id == "brute_in_garage" {
        lines.push("Final phase: over. The garage is no longer an active boss room.".to_owned());
    }

    lines
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
    use crate::game::actions::{EncounterKind, GameAction, GameEvent, ItemUseEffect};
    use crate::game::generation::generate_new_run;

    use super::{ROLLING_SUMMARY_LIMIT, apply_action};

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
    fn loud_actions_raise_noise_and_barricaded_wait_lowers_it() {
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
        assert_eq!(state.noise_level, 2);
        assert!(
            unlock
                .lines
                .iter()
                .any(|line| line == "Noise rises to Loud.")
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
        assert_eq!(state.noise_level, 3);
        assert!(
            barricade
                .lines
                .iter()
                .any(|line| line == "Noise rises to Swarming.")
        );

        let calm_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.noise_level, 2);
        assert!(
            calm_wait
                .lines
                .iter()
                .any(|line| line == "Noise settles to Loud.")
        );
    }

    #[test]
    fn exposed_pressure_and_retaliation_scale_up_at_high_noise() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 8);
        assert!(wait.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken {
                amount: 2,
                remaining_hp: 8
            }
        )));

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
