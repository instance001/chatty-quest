use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, GameEvent};
use super::context::movement_context_lines;
use super::items::try_unlock_with_item;
use super::queries::{find_location, find_location_by_name_or_id, is_location_locked};
use super::state::RunState;

pub(super) fn handle_move(
    state: &mut RunState,
    bundle: &DatapackBundle,
    destination: &str,
) -> ActionOutcome {
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

pub(super) fn handle_unlock(
    state: &mut RunState,
    bundle: &DatapackBundle,
    target: &str,
) -> ActionOutcome {
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

pub(super) fn handle_barricade(
    state: &mut RunState,
    bundle: &DatapackBundle,
    target: &str,
) -> ActionOutcome {
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
