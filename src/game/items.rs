use crate::data::datapacks::{DatapackBundle, LocationTemplate};

use super::actions::{ActionOutcome, GameEvent, ItemUseEffect};
use super::queries::{find_item, find_location, matches_name};
use super::state::{InventoryEntry, RunState};

pub(super) fn handle_take(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item_name: &str,
) -> ActionOutcome {
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

pub(super) fn handle_equip(state: &mut RunState, item_name: &str) -> ActionOutcome {
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

pub(super) fn handle_use(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item_name: &str,
) -> ActionOutcome {
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

pub(super) fn try_unlock_with_item(
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
    location: &LocationTemplate,
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
