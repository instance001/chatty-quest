use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, GameAction, GameEvent};
use super::inspect::handle_inspect;
use super::items::handle_equip;
use super::movement::handle_move;
use super::queries::describe_current_location;
use super::state::RunState;

pub(super) fn apply_epilogue_action(
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
