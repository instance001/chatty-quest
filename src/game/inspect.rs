use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, GameEvent};
use super::capabilities::EngineCapability;
use super::context::{inspect_boss_state_lines, inspect_enemy_state_lines, item_context_lines};
use super::queries::{
    describe_current_location, describe_location, find_boss_by_name_or_id, find_enemy,
    find_enemy_by_name_or_id, find_item_by_name_or_id, find_location_by_name_or_id, matches_name,
    unlock_targets_for_item,
};
use super::state::{InventoryEntry, RunState};

fn find_runtime_item<'a>(
    entries: &'a [InventoryEntry],
    target: &str,
) -> Option<&'a InventoryEntry> {
    for entry in entries {
        if matches_name(target, &entry.instance_id, &entry.name)
            || matches_name(target, &entry.id, &entry.name)
        {
            return Some(entry);
        }
        if let Some(found) = find_runtime_item(&entry.inventory, target) {
            return Some(found);
        }
    }
    None
}

fn append_inventory_contract_lines(
    lines: &mut Vec<String>,
    actor: &super::state::ActorState,
    inventory: &[InventoryEntry],
    equipped_item_id: Option<&str>,
) {
    if !actor.has_capability(EngineCapability::Inventory) {
        return;
    }
    if inventory.is_empty() {
        lines.push("Inventory: empty.".to_owned());
    } else {
        lines.push(format!(
            "Inventory: {}.",
            inventory
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(item_id) = equipped_item_id {
        lines.push(format!("Equipped item: {item_id}."));
    }
}

pub(super) fn handle_inspect(
    state: &RunState,
    bundle: &DatapackBundle,
    target: &str,
) -> ActionOutcome {
    if target == "room" || target == "area" || target == "location" {
        return ActionOutcome {
            events: vec![GameEvent::LocationLooked {
                origin_id: state.player_entity_id.clone(),
                location_id: state.current_location_id.clone(),
            }],
            lines: describe_current_location(state, bundle),
        };
    }

    if let Some(location) = find_location_by_name_or_id(bundle, target) {
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                origin_id: state.player_entity_id.clone(),
                target: location.id.clone(),
            }],
            lines: describe_location(state, bundle, location),
        };
    }

    let runtime_item = state
        .controlled_inventory()
        .and_then(|inventory| find_runtime_item(inventory, target))
        .or_else(|| {
            state
                .location_items
                .get(&state.current_location_id)
                .into_iter()
                .flatten()
                .find_map(|item_id| {
                    state.world_item_instances.get(item_id).filter(|item| {
                        matches_name(target, &item.instance_id, &item.name)
                            || matches_name(target, &item.id, &item.name)
                    })
                })
        });
    if let Some(item) = runtime_item {
        let mut lines = vec![format!("{}: {}", item.name, item.description)];
        if let Some(actor) = &item.actor {
            if actor.has_health_contract()
                && let Some(hp) = actor.current_hp
            {
                lines.push(format!("Health: {} / {}.", hp, actor.max_hp.unwrap_or(hp)));
            }
            if actor.is_combatant() {
                lines.push("Combat capability: present.".to_owned());
            }
            append_inventory_contract_lines(
                &mut lines,
                actor,
                &item.inventory,
                item.equipped_item_id.as_deref(),
            );
        }
        if let Some(template) = find_item_by_name_or_id(bundle, item.effective_template_id()) {
            lines.extend(item_context_lines(state, template));
        }
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                origin_id: state.player_entity_id.clone(),
                target: item.instance_id.clone(),
            }],
            lines,
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
                origin_id: state.player_entity_id.clone(),
                target: item.id.clone(),
            }],
            lines,
        };
    }

    if let Some(npc) = state
        .location_npcs
        .get(&state.current_location_id)
        .into_iter()
        .flatten()
        .find(|npc| {
            npc.lifecycle == super::state::InstanceLifecycle::Active
                && matches_name(target, &npc.template_id, &npc.name)
        })
    {
        let mut lines = vec![format!("{}: {}", npc.name, npc.description)];
        lines.push(format!(
            "Disposition: {:?}. Faction: {}.",
            npc.disposition, npc.faction
        ));
        if npc.actor.has_health_contract()
            && let Some(hp) = npc.actor.current_hp
        {
            lines.push(format!(
                "Health: {} / {}.",
                hp,
                npc.actor.max_hp.unwrap_or(hp)
            ));
        }
        if npc.actor.is_combatant() {
            lines.push(format!(
                "Combat capability: {}.",
                if npc.disposition == super::state::ActorDisposition::Hostile {
                    "hostile and active"
                } else {
                    "present but not hostile"
                }
            ));
        }
        if !npc.quest_objective_ids.is_empty() {
            lines.push(format!(
                "Quest offers: {}.",
                npc.quest_objective_ids.join(", ")
            ));
        }
        append_inventory_contract_lines(
            &mut lines,
            &npc.actor,
            &npc.inventory,
            npc.equipped_item_id.as_deref(),
        );
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                origin_id: state.player_entity_id.clone(),
                target: npc.template_id.clone(),
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
            events: vec![GameEvent::Inspected {
                origin_id: state.player_entity_id.clone(),
                target: enemy_id,
            }],
            lines,
        };
    }

    if let Some(enemy) = find_enemy_by_name_or_id(bundle, target) {
        let mut lines = vec![format!("{}: {}", enemy.name, enemy.description)];
        lines.extend(inspect_enemy_state_lines(state, bundle, &enemy.id));
        return ActionOutcome {
            events: vec![GameEvent::Inspected {
                origin_id: state.player_entity_id.clone(),
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
                origin_id: state.player_entity_id.clone(),
                target: boss.id.clone(),
            }],
            lines,
        };
    }

    ActionOutcome {
        events: vec![GameEvent::Inspected {
            origin_id: state.player_entity_id.clone(),
            target: target.to_owned(),
        }],
        lines: vec!["There is nothing useful to inspect by that name.".to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use super::handle_inspect;
    use crate::game::{ActorDisposition, ActorState, GameEvent, InstanceLifecycle, NpcInstance};

    #[test]
    fn inspecting_uses_the_controlled_actors_inventory_endpoint() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let item = state.inventory.remove(0);
        let item_name = item.name.clone();
        let item_instance_id = item.instance_id.clone();
        let location_id = state.current_location_id.clone();
        let actor_id = "controlled_pack_actor".to_owned();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "controlled_pack_actor".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Controlled Pack Actor".to_owned(),
                description: "Carries the inspected item.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: vec![item],
                equipped_item_id: None,
            }],
        );
        state.player_entity_id = actor_id;

        let outcome = handle_inspect(&state, &bundle, &item_name);

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::Inspected { target, .. }] if target == &item_instance_id
        ));
    }

    #[test]
    fn inspecting_an_inventory_actor_reports_authoritative_contents() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let item = state.inventory.remove(0);
        let item_name = item.name.clone();
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "pack_actor".to_owned(),
                template_id: "pack_actor".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Pack Actor".to_owned(),
                description: "Carries a visible item.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: vec![item],
                equipped_item_id: None,
            }],
        );

        let outcome = handle_inspect(&state, &bundle, "pack actor");
        assert!(
            outcome
                .lines
                .iter()
                .any(|line| line == &format!("Inventory: {item_name}."))
        );
    }

    #[test]
    fn inspecting_an_actor_capable_item_reports_its_real_contents() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let mut container = state.inventory.remove(0);
        let carried = state.inventory.remove(0);
        let carried_name = carried.name.clone();
        let container_name = container.name.clone();
        container.actor = Some(ActorState {
            capabilities: vec!["inventory".to_owned()],
            ..Default::default()
        });
        container.inventory.push(carried);
        state.inventory.push(container);

        let outcome = handle_inspect(&state, &bundle, &container_name);
        assert!(
            outcome
                .lines
                .iter()
                .any(|line| line == &format!("Inventory: {carried_name}."))
        );
    }

    #[test]
    fn inspection_hides_stale_payloads_from_disabled_capability_branches() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut state = crate::game::generation::generate_new_run(&bundle).state;
        let mut item = state.inventory.remove(0);
        let item_name = item.name.clone();
        item.actor = Some(ActorState {
            current_hp: Some(6),
            max_hp: Some(10),
            damage: Some(4),
            combat: true,
            capabilities: vec!["inventory".to_owned()],
            ..Default::default()
        });
        state.inventory.push(item);

        let outcome = handle_inspect(&state, &bundle, &item_name);

        assert!(
            outcome
                .lines
                .iter()
                .all(|line| !line.starts_with("Health:"))
        );
        assert!(
            outcome
                .lines
                .iter()
                .all(|line| line != "Combat capability: present.")
        );
    }
}
