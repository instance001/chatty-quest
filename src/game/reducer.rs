use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, EncounterKind, GameAction, GameEvent, ItemUseEffect};
use super::queries::{
    describe_current_location, equipped_damage, find_boss, find_boss_by_name_or_id, find_enemy,
    find_enemy_by_name_or_id, find_item, find_item_by_name_or_id, find_location,
    find_location_by_name_or_id, matches_name,
};
use super::state::{InventoryEntry, RunState};

pub fn apply_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: GameAction,
) -> ActionOutcome {
    let was_alive = state.hp > 0;
    let mut outcome = match action {
        GameAction::Help => ActionOutcome {
            events: vec![GameEvent::HelpShown],
            lines: vec![
                "Commands: help, look, go <location>, inspect <thing>, take <item>, equip <item>, use <item>, attack, wait."
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
        GameAction::Take { item_name } => handle_take(state, bundle, &item_name),
        GameAction::Equip { item_name } => handle_equip(state, &item_name),
        GameAction::Use { item_name } => handle_use(state, bundle, &item_name),
        GameAction::Attack => handle_attack(state, bundle),
        GameAction::Wait => handle_wait(state, bundle),
    };

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
            lines: vec![format!("{}: {}", location.name, location.description)],
        };
    }

    if let Some(item) = find_item_by_name_or_id(bundle, target) {
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                target: item.id.clone(),
            }],
            lines: vec![format!("{}: {}", item.name, item.description)],
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
    let completed_now = state
        .bosses_defeated
        .contains(&state.active_objective.target_boss_id);
    let just_completed = completed_now && !state.active_objective.completed;
    state.active_objective.completed = completed_now;
    just_completed
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
        GameEvent::Inspected { target } => Some(format!("Inspected '{}'.", target)),
        GameEvent::ItemTaken { item_id } => Some(format!("Took item '{}'.", item_id)),
        GameEvent::ItemEquipped { item_id } => Some(format!("Equipped item '{}'.", item_id)),
        GameEvent::ItemUsed { item_id, effect } => match effect {
            ItemUseEffect::Healing { amount } => Some(format!(
                "Used item '{}' for healing {} HP.",
                item_id, amount
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
        assert!(final_outcome.lines.iter().any(|line| line == "You win."));
    }
}
