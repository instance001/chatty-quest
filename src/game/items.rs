use crate::data::datapacks::{DatapackBundle, LocationTemplate};

use super::actions::{ActionOutcome, GameEvent, ItemUseEffect};
use super::discovery::reveal_current_connections;
use super::queries::{find_item, find_location, matches_name};
use super::state::ItemOwner;
use super::state::{InventoryEntry, RunState};

pub(super) fn handle_take(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item_name: &str,
) -> ActionOutcome {
    let location_id = state.current_location_id.clone();
    let Some(location_items) = state.location_items.get(&location_id) else {
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
                .or_else(|| {
                    state.world_item_instances.get(*item_id).map(|item| {
                        matches_name(item_name, &item.id, &item.name)
                            || matches_name(item_name, &item.instance_id, &item.name)
                            || matches_name(item_name, item.effective_template_id(), &item.name)
                    })
                })
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

    handle_transfer_item(
        state,
        bundle,
        &item_id,
        ItemOwner::Location(location_id),
        state.controlled_inventory_owner(),
    )
}

/// Resolves the one exceptional ownership edge that needs authored datapack data: a location may
/// contain either a concrete runtime item instance or an authored item reference. Every other
/// transfer is fully runtime-owned by `RunState::transfer_owned_item`.
pub(super) fn handle_transfer_item(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item_id: &str,
    origin: ItemOwner,
    destination: ItemOwner,
) -> ActionOutcome {
    let ItemOwner::Location(location_id) = &origin else {
        return match state.transfer_owned_item(item_id, &origin, &destination) {
            Ok(()) => ActionOutcome {
                events: vec![GameEvent::ItemTransferred {
                    item_id: item_id.to_owned(),
                    origin,
                    destination,
                }],
                lines: vec![format!("{item_id} transferred.")],
            },
            Err(reason) => ActionOutcome {
                events: vec![GameEvent::ActionRejected {
                    reason: reason.clone(),
                }],
                lines: vec![reason],
            },
        };
    };
    if !state.accepts_item_transfer(&destination) {
        let reason = "Destination does not have the inventory capability.".to_owned();
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        };
    }
    if location_id != &state.current_location_id {
        let reason = "You can only take an item from your current location.".to_owned();
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        };
    }
    let present = state
        .location_items
        .get(location_id)
        .is_some_and(|items| items.iter().any(|entry| entry == item_id));
    if !present {
        let reason = "Origin does not own that item.".to_owned();
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        };
    }

    let item_template_id = state
        .world_item_instances
        .get(item_id)
        .map(|entry| entry.effective_template_id().to_owned())
        .unwrap_or_else(|| item_id.to_owned());
    let item_template = find_item(bundle, &item_template_id);
    if item_template.is_none() && !state.world_item_instances.contains_key(item_id) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "The item data could not be resolved.".to_owned(),
            }],
            lines: vec!["The item data could not be resolved.".to_owned()],
        };
    };

    state
        .location_items
        .get_mut(location_id)
        .expect("validated location item ownership")
        .retain(|entry| entry != item_id);
    let mut entry = state
        .world_item_instances
        .remove(item_id)
        .unwrap_or_else(|| {
            let item = item_template.expect("validated authored location item");
            InventoryEntry {
                id: item.id.clone(),
                instance_id: format!("authored_item_{}", item.id),
                template_id: item.id.clone(),
                bucket: super::state::ItemBucket::Inventory,
                location_id: None,
                lifecycle: super::state::InstanceLifecycle::Active,
                name: item.name.clone(),
                description: item.description.clone(),
                damage: item.damage,
                origin: super::state::ItemOrigin::Authored,
                condition: None,
                tags: item.tags.clone(),
                actor: item.actor.as_ref().map(|actor| {
                    let mut state = super::state::ActorState::configured(
                        actor.hp,
                        actor.damage,
                        actor.can_hear,
                        actor.can_see,
                        actor.can_move,
                        actor.combat,
                    );
                    state.apply_declared_capabilities(&item.capabilities);
                    state
                }),
                disposition: item
                    .actor
                    .as_ref()
                    .map_or(super::state::ActorDisposition::Neutral, |actor| {
                        super::state::ActorDisposition::from_template_value(&actor.disposition)
                    }),
                inventory: Vec::new(),
                equipped_item_id: None,
            }
        });
    entry.bucket = super::state::ItemBucket::Inventory;
    entry.location_id = None;
    let transferred_item_id = entry.id.clone();
    let item_name = entry.name.clone();
    let destination_is_controlled = destination == state.controlled_inventory_owner();
    let destination_label = destination.to_string();
    state
        .entity_inventory_mut(&destination)
        .expect("validated transfer destination inventory")
        .push(entry);

    ActionOutcome {
        events: vec![GameEvent::ItemTransferred {
            item_id: transferred_item_id,
            origin,
            destination,
        }],
        lines: {
            let mut lines = vec![if destination_is_controlled {
                format!("You take the {item_name}.")
            } else {
                format!("{item_name} is transferred to {destination_label}.")
            }];
            if destination_is_controlled
                && let Some(pickup_line) =
                    item_template.and_then(|item| item.pickup_line.as_deref())
            {
                lines.push(pickup_line.to_owned());
            }
            lines
        },
    }
}

pub(super) fn handle_equip(state: &mut RunState, item_name: &str) -> ActionOutcome {
    if !state.controlled_actor_can_carry_inventory() {
        return inventory_capability_rejected();
    }
    let owner = state.controlled_inventory_owner();
    let Some(item) = state
        .controlled_inventory()
        .and_then(|inventory| {
            inventory
                .iter()
                .find(|item| matches_name(item_name, &item.id, &item.name))
        })
        .cloned()
    else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You do not have that item.".to_owned(),
            }],
            lines: vec!["You do not have that item.".to_owned()],
        };
    };

    if let Err(reason) = state.equip_owned_item(&owner, &item.id) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        };
    }

    ActionOutcome {
        events: vec![GameEvent::ItemEquipped {
            origin_id: state.player_entity_id.clone(),
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
    if !state.controlled_actor_can_carry_inventory() {
        return inventory_capability_rejected();
    }
    let owner = state.controlled_inventory_owner();
    let Some(item) = state
        .controlled_inventory()
        .and_then(|inventory| {
            inventory
                .iter()
                .find(|item| matches_name(item_name, &item.id, &item.name))
        })
        .cloned()
    else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "You do not have that item.".to_owned(),
            }],
            lines: vec!["You do not have that item.".to_owned()],
        };
    };

    let Some(template) = find_item(bundle, item.effective_template_id()) else {
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
        let origin_id = state.player_entity_id.clone();
        if !state.runtime_actor_has_tag(&origin_id, "alive") {
            let reason = "This ordinary healing item can only heal an actor with the 'alive' tag; it cannot revive an incapacitated actor."
                .to_owned();
            return ActionOutcome {
                events: vec![GameEvent::ActionRejected {
                    reason: reason.clone(),
                }],
                lines: vec![reason],
            };
        }
        let (_, restored, remaining_hp) = match state.restore_runtime_actor_health(&origin_id, 4) {
            Ok(result) => result,
            Err(reason) => {
                return ActionOutcome {
                    events: vec![GameEvent::ActionRejected {
                        reason: reason.clone(),
                    }],
                    lines: vec![reason],
                };
            }
        };
        if let Err(reason) = state.consume_owned_item(&item.id, &owner) {
            return ActionOutcome {
                events: vec![GameEvent::ActionRejected {
                    reason: reason.clone(),
                }],
                lines: vec![reason],
            };
        }

        return ActionOutcome {
            events: vec![
                GameEvent::ItemUsed {
                    origin_id: origin_id.clone(),
                    item_id: item.id.clone(),
                    effect: ItemUseEffect::Healing { amount: restored },
                },
                GameEvent::HealthRestored {
                    origin_id: origin_id.clone(),
                    target_id: origin_id,
                    amount: restored,
                    remaining_hp,
                },
            ],
            lines: vec![
                format!("You use the {}.", item.name),
                format!("HP rises to {}.", remaining_hp),
                "You buy yourself a few ragged minutes of competence.".to_owned(),
            ],
        };
    }

    ActionOutcome {
        events: vec![GameEvent::ItemUsed {
            origin_id: state.player_entity_id.clone(),
            item_id: item.id.clone(),
            effect: ItemUseEffect::NoEffect,
        }],
        lines: vec![format!(
            "You fumble with the {}, but it has no usable v0.1 effect.",
            item.name
        )],
    }
}

fn inventory_capability_rejected() -> ActionOutcome {
    let reason = "The controlled actor lacks the inventory capability contract.".to_owned();
    ActionOutcome {
        events: vec![GameEvent::ActionRejected {
            reason: reason.clone(),
        }],
        lines: vec![reason],
    }
}

fn reveal_connected_locations(
    state: &mut RunState,
    bundle: &DatapackBundle,
    item: &InventoryEntry,
) -> ActionOutcome {
    let newly_known = match reveal_current_connections(state, bundle) {
        Ok(newly_known) => newly_known,
        Err(reason) => {
            return ActionOutcome {
                events: vec![GameEvent::ActionRejected {
                    reason: reason.clone(),
                }],
                lines: vec![reason],
            };
        }
    };

    let item_template = find_item(bundle, item.effective_template_id());
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
            origin_id: state.player_entity_id.clone(),
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
    if let Some(instance) = state.location_instances.get_mut(&location.id) {
        instance.locked = false;
    }
    ActionOutcome {
        events: vec![
            GameEvent::ItemUsed {
                origin_id: state.player_entity_id.clone(),
                item_id: item_id.to_owned(),
                effect: ItemUseEffect::NoEffect,
            },
            GameEvent::LocationUnlocked {
                origin_id: state.player_entity_id.clone(),
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

#[cfg(test)]
mod tests {
    use super::{handle_take, handle_transfer_item};
    use crate::game::state::{
        ActorDisposition, ActorState, InstanceLifecycle, ItemOrigin, ItemOwner, NpcInstance,
    };
    use crate::game::{GameAction, GameEvent, apply_action};

    #[test]
    fn a_controlled_actor_without_inventory_cannot_use_preexisting_item_storage() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let carried_item = state.inventory.remove(0);
        let carried_item_id = carried_item.id.clone();
        let item_name = carried_item.name.clone();
        let actor_id = "inventory_disabled_fixture".to_owned();
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "inventory_disabled_fixture".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Inventory-disabled fixture".to_owned(),
                description: "Has stale storage but no inventory capability.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Neutral,
                actor: ActorState::default(),
                quest_objective_ids: Vec::new(),
                inventory: vec![carried_item],
                equipped_item_id: None,
            }],
        );
        state.player_entity_id = actor_id;

        assert!(state.controlled_inventory().is_none());
        let outcome = apply_action(&mut state, &bundle, GameAction::Equip { item_name });
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }]
                if reason == "The controlled actor lacks the inventory capability contract."
        ));
        assert_eq!(
            state
                .transfer_owned_item(
                    &carried_item_id,
                    &ItemOwner::Entity("inventory_disabled_fixture".to_owned()),
                    &ItemOwner::Location(state.current_location_id.clone()),
                )
                .expect_err("the shared transfer primitive also gates an entity source"),
            "Origin does not have the inventory capability."
        );
    }

    #[test]
    fn proposal_created_runtime_item_can_be_dropped_and_taken_again() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let mut item = state.inventory.remove(0);
        item.id = "session_relic".to_owned();
        item.template_id = "session_relic".to_owned();
        item.instance_id = "session_relic_1".to_owned();
        item.name = "Session Relic".to_owned();
        item.description = "A runtime-only test item.".to_owned();
        item.origin = ItemOrigin::NarratorApproved;
        state.inventory.push(item.clone());
        let location_id = state.current_location_id.clone();

        state
            .transfer_owned_item(
                &item.id,
                &ItemOwner::Player,
                &ItemOwner::Location(location_id.clone()),
            )
            .expect("a runtime item can be dropped into the world bucket");
        assert!(
            !state
                .inventory
                .iter()
                .any(|entry| entry.id == "session_relic")
        );

        let outcome = handle_take(&mut state, &bundle, "session_relic_1");
        assert!(outcome.events.iter().any(|event| {
            matches!(event, crate::game::GameEvent::ItemTransferred { item_id, origin: ItemOwner::Location(source), destination: ItemOwner::Player }
                if item_id == "session_relic" && source == &location_id)
        }));
        assert!(state.inventory.iter().any(|entry| {
            entry.id == "session_relic" && entry.origin == ItemOrigin::NarratorApproved
        }));
    }

    #[test]
    fn world_item_can_transfer_directly_to_an_actor_capable_item() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let source = state.inventory.remove(0);
        let source_id = source.id.clone();
        let source_instance_id = source.instance_id.clone();
        let mut container = state.inventory.remove(0);
        let container_id = container.instance_id.clone();
        let container_item_id = container.id.clone();
        container.actor = Some(crate::game::ActorState {
            capabilities: vec!["inventory".to_owned()],
            ..Default::default()
        });
        state.inventory.push(source);
        state.inventory.push(container);
        let location_id = state.current_location_id.clone();
        for item_id in [&source_id, &container_item_id] {
            state
                .transfer_owned_item(
                    item_id,
                    &ItemOwner::Player,
                    &ItemOwner::Location(location_id.clone()),
                )
                .expect("items can enter the world bucket");
        }

        let outcome = handle_transfer_item(
            &mut state,
            &bundle,
            &source_instance_id,
            ItemOwner::Location(location_id),
            ItemOwner::Entity(container_id.clone()),
        );
        assert!(matches!(
            outcome.events.as_slice(),
            [crate::game::GameEvent::ItemTransferred { destination: ItemOwner::Entity(id), .. }] if id == &container_id
        ));
        assert!(
            state
                .entity_inventory(&ItemOwner::Entity(container_id))
                .is_some_and(|inventory| inventory.iter().any(|item| item.id == source_id))
        );
    }
}
