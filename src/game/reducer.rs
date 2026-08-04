use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, GameAction, GameEvent};
use super::combat::handle_attack;
use super::epilogue::apply_epilogue_action;
use super::inspect::handle_inspect;
use super::items::{handle_equip, handle_take, handle_use};
use super::movement::{handle_barricade, handle_move, handle_unlock};
use super::noise::apply_noise_for_action;
use super::objectives::{
    objective_condition_statuses, objective_progress_lines, update_objective_completion,
};
use super::queries::describe_current_location;
use super::spawned_ai::apply_spawned_enemy_turns;
use super::state::RunState;
use super::summary::append_rolling_summary;
use super::wait::handle_wait;

#[cfg(test)]
use super::spawned_ai::{
    SpawnedEnemyStep, deterministic_hazard_break_roll, spawned_enemy_next_step,
};

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

    if accepted_turn {
        apply_noise_for_action(state, bundle, &action_for_noise, &mut outcome);
    }
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

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{
        EncounterKind, GameAction, GameEvent, ItemUseEffect, MovementHazardKind,
    };
    use crate::game::generation::generate_new_run;

    use super::{
        SpawnedEnemyStep, advance_turn_index, apply_action, deterministic_hazard_break_roll,
        spawned_enemy_next_step,
    };
    use crate::game::summary::ROLLING_SUMMARY_LIMIT;

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
