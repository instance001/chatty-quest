use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, EncounterKind, GameAction, GameEvent, ItemUseEffect};
use super::queries::{
    describe_current_location, describe_location, equipped_damage, find_boss,
    find_boss_by_name_or_id, find_enemy, find_enemy_by_name_or_id, find_item,
    find_item_by_name_or_id, find_location, find_location_by_name_or_id, is_location_locked,
    matches_name, unlock_targets_for_item,
};
use super::state::{InventoryEntry, RunState};

pub fn apply_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: GameAction,
) -> ActionOutcome {
    let was_alive = state.hp > 0;
    let objective_before = objective_condition_statuses(state);
    let mut outcome = match action {
        GameAction::Help => ActionOutcome {
            events: vec![GameEvent::HelpShown],
            lines: vec![
                "Commands: help, look, go <location>, unlock <location>, inspect <thing>, take <item>, equip <item>, use <item>, attack, wait."
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
        GameAction::Inspect { target } => handle_inspect(state, bundle, &target),
        GameAction::Take { item_name } => handle_take(state, bundle, &item_name),
        GameAction::Equip { item_name } => handle_equip(state, &item_name),
        GameAction::Use { item_name } => handle_use(state, bundle, &item_name),
        GameAction::Attack => handle_attack(state, bundle),
        GameAction::Wait => handle_wait(state, bundle),
    };

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

    let summary_lines = rolling_summary_lines(&outcome.events, &outcome.lines);
    state.rolling_summary.extend(summary_lines);
    ActionOutcome {
        events: outcome.events,
        lines: outcome.lines,
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

    ActionOutcome {
        events: vec![GameEvent::Moved {
            from_location_id,
            to_location_id: destination_location.id.clone(),
        }],
        lines: vec![
            format!("You move to {}.", destination_location.name),
            destination_location.description.clone(),
        ],
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
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: item.id.clone(),
            }],
            lines,
        };
    }

    if let Some(enemy) = find_enemy_by_name_or_id(bundle, target) {
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: enemy.id.clone(),
            }],
            lines: vec![format!("{}: {}", enemy.name, enemy.description)],
        };
    }

    if let Some(boss) = find_boss_by_name_or_id(bundle, target) {
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: boss.id.clone(),
            }],
            lines: vec![format!("{}: {}", boss.name, boss.description)],
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
        lines: vec![format!("You take the {}.", item.name)],
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
        let player_damage = equipped_damage(state).max(1);
        let enemy_damage = state.enemy_hp.entry(enemy_id.clone()).or_insert(0);
        *enemy_damage -= player_damage;

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
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
        } else {
            let retaliation = find_enemy(bundle, &enemy_id)
                .map(|enemy| enemy.damage)
                .unwrap_or(1);
            state.hp = (state.hp - retaliation).max(0);
            events.push(GameEvent::DamageTaken {
                amount: retaliation,
                remaining_hp: state.hp,
            });
            lines.push(format!("The enemy hits back for {} damage.", retaliation));
            lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
        }

        return ActionOutcome { events, lines };
    }

    if let Some(boss_id) = boss_here {
        let player_damage = equipped_damage(state).max(1);
        let boss_damage = state.boss_hp.entry(boss_id.clone()).or_insert(0);
        *boss_damage -= player_damage;

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        let mut events = vec![GameEvent::AttackResolved {
            target_id: boss_id.clone(),
            target_kind: EncounterKind::Boss,
            damage: player_damage,
            defeated: *boss_damage <= 0,
        }];

        if *boss_damage <= 0 {
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
        } else {
            let retaliation = find_boss(bundle, &boss_id)
                .map(|boss| boss.damage)
                .unwrap_or(2);
            state.hp = (state.hp - retaliation).max(0);
            events.push(GameEvent::DamageTaken {
                amount: retaliation,
                remaining_hp: state.hp,
            });
            lines.push(format!("The boss smashes back for {} damage.", retaliation));
            lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
        }

        return ActionOutcome { events, lines };
    }

    ActionOutcome {
        events: vec![GameEvent::AttackWhiff],
        lines: vec!["You swing at the air with admirable commitment.".to_owned()],
    }
}

fn handle_wait(state: &RunState, bundle: &DatapackBundle) -> ActionOutcome {
    let location_name = find_location(bundle, &state.current_location_id)
        .map(|location| location.name.clone())
        .unwrap_or_else(|| state.current_location_id.clone());

    ActionOutcome {
        events: vec![GameEvent::Waited {
            location_id: state.current_location_id.clone(),
        }],
        lines: vec![format!(
            "You wait at {} and listen to the property complain around you.",
            location_name
        )],
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

    use super::apply_action;

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
}
