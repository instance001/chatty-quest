use std::collections::{HashMap, HashSet};

use crate::data::datapacks::{
    ActorCapabilitiesTemplate, DatapackBundle, ItemTemplate, ObjectiveTemplate,
};

use super::state::AbilityGrantSource;
use super::state::{
    AbilityGrant, ActorDisposition, ActorState, InventoryEntry, ItemOwner, NpcInstance,
    ObjectiveState, RunState,
};

#[derive(Clone, Debug)]
pub struct GeneratedRun {
    pub state: RunState,
    pub log_lines: Vec<String>,
}

pub fn generate_new_run(bundle: &DatapackBundle) -> GeneratedRun {
    let starter_items: Vec<InventoryEntry> = bundle
        .items
        .iter()
        .filter(|item| item.tags.iter().any(|tag| tag == "starter_item"))
        .map(to_inventory_entry)
        .collect();

    let equipped_item_id = starter_items.first().map(|item| item.id.clone());

    let active_objective_template = select_objective(&bundle.objectives);
    let active_objective = ObjectiveState {
        instance_id: format!("authored_objective_{}", active_objective_template.id),
        template_id: active_objective_template.id.clone(),
        lifecycle: super::state::InstanceLifecycle::Active,
        bucket: super::state::ObjectiveBucket::Active,
        id: active_objective_template.id.clone(),
        name: active_objective_template.name.clone(),
        description: active_objective_template.description.clone(),
        target_boss_id: active_objective_template.target_boss_id.clone(),
        required_item_id: active_objective_template.required_item_id.clone(),
        required_location_id: active_objective_template.required_location_id.clone(),
        completed: false,
    };

    let current_location_id = bundle.rules.starting_location.clone();
    let known_locations = HashSet::from([current_location_id.clone()]);
    let visited_locations = HashSet::from([current_location_id.clone()]);
    let location_instances = bundle
        .locations
        .iter()
        .map(|location| {
            (
                location.id.clone(),
                super::state::LocationInstance {
                    instance_id: format!("authored_location_{}", location.id),
                    template_id: location.id.clone(),
                    known: location.id == current_location_id,
                    visited: location.id == current_location_id,
                    locked: location.locked,
                    lifecycle: super::state::InstanceLifecycle::Active,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let enemies_alive = bundle
        .enemies
        .iter()
        .map(|enemy| enemy.id.clone())
        .collect::<HashSet<_>>();
    let enemies_defeated = HashSet::new();
    let enemy_instances = bundle
        .enemies
        .iter()
        .filter_map(|enemy| {
            bundle.locations.iter().find_map(|location| {
                location.enemies.contains(&enemy.id).then(|| {
                    (
                        enemy.id.clone(),
                        super::state::EncounterInstance {
                            instance_id: format!("authored_enemy_{}", enemy.id),
                            template_id: enemy.id.clone(),
                            location_id: location.id.clone(),
                            bucket: super::state::EncounterBucket::Live,
                            lifecycle: super::state::InstanceLifecycle::Active,
                            current_hp: enemy.hp,
                            max_hp: enemy.hp,
                            provenance: "authored_datapack".to_owned(),
                            name: enemy.name.clone(),
                            description: enemy.description.clone(),
                            damage: enemy.damage,
                            traversal_cost: enemy.traversal_cost,
                            attack_cost: enemy.attack_cost,
                            tags: enemy.tags.clone(),
                            capabilities: enemy.capabilities.clone(),
                        },
                    )
                })
            })
        })
        .collect::<HashMap<_, _>>();
    let enemy_hp = bundle
        .enemies
        .iter()
        .map(|enemy| (enemy.id.clone(), enemy.hp))
        .collect::<HashMap<_, _>>();
    let bosses_alive = bundle
        .bosses
        .iter()
        .map(|boss| boss.id.clone())
        .collect::<HashSet<_>>();
    let bosses_defeated = HashSet::new();
    let boss_instances = bundle
        .bosses
        .iter()
        .filter_map(|boss| {
            bundle.locations.iter().find_map(|location| {
                location.bosses.contains(&boss.id).then(|| {
                    (
                        boss.id.clone(),
                        super::state::EncounterInstance {
                            instance_id: format!("authored_boss_{}", boss.id),
                            template_id: boss.id.clone(),
                            location_id: location.id.clone(),
                            bucket: super::state::EncounterBucket::Live,
                            lifecycle: super::state::InstanceLifecycle::Active,
                            current_hp: boss.hp,
                            max_hp: boss.hp,
                            provenance: "authored_datapack".to_owned(),
                            name: boss.name.clone(),
                            description: boss.description.clone(),
                            damage: boss.damage,
                            traversal_cost: boss.traversal_cost,
                            attack_cost: boss.attack_cost,
                            tags: boss.tags.clone(),
                            capabilities: boss.capabilities.clone(),
                        },
                    )
                })
            })
        })
        .collect::<HashMap<_, _>>();
    let boss_hp = bundle
        .bosses
        .iter()
        .map(|boss| (boss.id.clone(), boss.hp))
        .collect::<HashMap<_, _>>();

    let location_items = bundle
        .locations
        .iter()
        .map(|location| (location.id.clone(), location.items.clone()))
        .collect::<HashMap<_, _>>();
    let world_item_instances = bundle
        .locations
        .iter()
        .flat_map(|location| {
            location.items.iter().filter_map(move |template_id| {
                bundle
                    .items
                    .iter()
                    .find(|item| &item.id == template_id)
                    .map(|item| {
                        let instance_id = format!("authored_world_{}_{}", location.id, item.id);
                        (
                            instance_id.clone(),
                            InventoryEntry {
                                id: item.id.clone(),
                                instance_id,
                                template_id: item.id.clone(),
                                bucket: super::state::ItemBucket::World,
                                location_id: Some(location.id.clone()),
                                lifecycle: super::state::InstanceLifecycle::Active,
                                name: item.name.clone(),
                                description: item.description.clone(),
                                damage: item.damage,
                                origin: super::state::ItemOrigin::Authored,
                                condition: None,
                                tags: item.tags.clone(),
                                actor: item
                                    .actor
                                    .as_ref()
                                    .map(|actor| actor_state(actor, &item.capabilities)),
                                disposition: item.actor.as_ref().map_or(
                                    ActorDisposition::Neutral,
                                    |actor| {
                                        ActorDisposition::from_template_value(&actor.disposition)
                                    },
                                ),
                                inventory: Vec::new(),
                                equipped_item_id: None,
                            },
                        )
                    })
            })
        })
        .collect::<HashMap<_, _>>();
    let location_enemies = bundle
        .locations
        .iter()
        .map(|location| (location.id.clone(), location.enemies.clone()))
        .collect::<HashMap<_, _>>();
    let location_bosses = bundle
        .locations
        .iter()
        .map(|location| (location.id.clone(), location.bosses.clone()))
        .collect::<HashMap<_, _>>();
    let location_npcs = bundle
        .locations
        .iter()
        .filter_map(|location| {
            let npcs = location
                .npcs
                .iter()
                .filter_map(|npc_id| {
                    bundle
                        .npcs
                        .iter()
                        .find(|npc| npc.id == *npc_id)
                        .map(|npc| NpcInstance {
                            instance_id: format!("authored_npc_{}", npc.id),
                            template_id: npc.id.clone(),
                            location_id: location.id.clone(),
                            lifecycle: super::state::InstanceLifecycle::Active,
                            provenance: "authored_datapack".to_owned(),
                            name: npc.name.clone(),
                            description: npc.description.clone(),
                            faction: npc.faction.clone(),
                            tags: npc.tags.clone(),
                            disposition: ActorDisposition::from_template_value(&npc.disposition),
                            actor: actor_state(&npc.actor, &npc.capabilities),
                            quest_objective_ids: npc.quest_objective_ids.clone(),
                            inventory: Vec::new(),
                            equipped_item_id: None,
                        })
                })
                .collect::<Vec<_>>();
            (!npcs.is_empty()).then(|| (location.id.clone(), npcs))
        })
        .collect::<HashMap<_, _>>();
    let locked_locations = bundle
        .locations
        .iter()
        .filter(|location| location.locked)
        .map(|location| location.id.clone())
        .collect::<HashSet<_>>();
    let barricaded_locations = HashSet::new();
    let broken_locked_locations = HashSet::new();

    let location_name = bundle
        .locations
        .iter()
        .find(|location| location.id == current_location_id)
        .map(|location| location.name.clone())
        .unwrap_or_else(|| current_location_id.clone());

    let mut log_lines = vec![
        format!("Scenario loaded: {}.", bundle.pack.display_name),
        format!("You begin at {}.", location_name),
        format!("Objective locked in: {}.", active_objective.name),
    ];
    if let Some(starter_hint_line) = bundle.rules.starter_hint_line.as_deref() {
        log_lines.push(starter_hint_line.to_owned());
    }

    if let Some(dm_style) = &bundle.dm_style {
        log_lines.push(format!("DM capsule: {}.", dm_style));
    }

    let mut state = RunState {
        player_entity_id: "player".to_owned(),
        player_tags: {
            let mut tags = vec!["player".to_owned()];
            tags.extend(bundle.rules.player_tags.clone());
            tags.sort();
            tags.dedup();
            tags
        },
        datapack_id: bundle.pack.id.clone(),
        datapack_display_name: bundle.pack.display_name.clone(),
        current_location_id,
        known_locations,
        visited_locations,
        location_instances,
        temporary_exits: Vec::new(),
        inventory: starter_items,
        equipped_item_id,
        player_media_enabled: true,
        status_effects: Vec::new(),
        location_npcs,
        social_states: HashMap::new(),
        progress_pools: HashMap::new(),
        actor_progress_pools: HashMap::new(),
        ability_grants: bundle
            .rules
            .starter_ability_ids
            .iter()
            .filter_map(|template_id| {
                bundle
                    .abilities
                    .iter()
                    .find(|ability| &ability.id == template_id)
                    .map(|ability| AbilityGrant {
                        instance_id: format!("starter_ability_{}", ability.id),
                        template_id: ability.id.clone(),
                        owner: ItemOwner::Player,
                        remaining_uses: ability.default_uses,
                        expires_at_turn: ability.duration_turns,
                        ready_at_turn: 0,
                        source: AbilityGrantSource::Innate,
                    })
            })
            .collect(),
        hp: 10,
        max_hp: 10,
        active_objective,
        additional_objectives: Vec::new(),
        enemies_alive,
        enemy_instances,
        enemies_defeated,
        enemy_hp,
        bosses_alive,
        boss_instances,
        bosses_defeated,
        boss_hp,
        location_items,
        world_item_instances,
        location_enemies,
        location_bosses,
        locked_locations,
        broken_locked_locations,
        barricaded_locations,
        turn_index: 0,
        actor_ready_at: HashMap::new(),
        last_ambient_proposal_turn: None,
        noise_level: 0,
        noise_spawn_count: 0,
        actor_noise_targets: HashMap::new(),
        actor_origins: HashMap::new(),
        actor_searching: HashSet::new(),
        actor_sight_targets: HashMap::new(),
        actor_sight_subjects: HashMap::new(),
        actor_sight_delays: HashMap::new(),
        scratch_item_admission_count: 0,
        boundary_response: bundle.rules.boundary_response.clone(),
        rolling_summary: vec![format!(
            "Run started for scenario '{}'.",
            bundle.pack.display_name
        )],
    };

    state.ability_grants.extend(
        state
            .inventory
            .iter()
            .chain(state.world_item_instances.values())
            .flat_map(|item| ability_grants_for_item(bundle, item)),
    );

    GeneratedRun { state, log_lines }
}

fn ability_grants_for_item(bundle: &DatapackBundle, item: &InventoryEntry) -> Vec<AbilityGrant> {
    let Some(item_template) = bundle
        .items
        .iter()
        .find(|template| template.id == item.effective_template_id())
    else {
        return Vec::new();
    };
    item_template
        .ability_grants
        .iter()
        .enumerate()
        .filter_map(|(index, grant)| {
            bundle
                .abilities
                .iter()
                .find(|ability| ability.id == grant.template_id)
                .map(|ability| AbilityGrant {
                    instance_id: format!(
                        "item_ability_{}_{}_{}",
                        item.instance_id, ability.id, index
                    ),
                    template_id: ability.id.clone(),
                    owner: ItemOwner::Entity(item.instance_id.clone()),
                    remaining_uses: grant.remaining_uses.or(ability.default_uses),
                    expires_at_turn: grant.duration_turns.or(ability.duration_turns),
                    ready_at_turn: 0,
                    source: AbilityGrantSource::Item,
                })
        })
        .collect()
}

fn to_inventory_entry(item: &ItemTemplate) -> InventoryEntry {
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
        actor: item
            .actor
            .as_ref()
            .map(|actor| actor_state(actor, &item.capabilities)),
        disposition: item
            .actor
            .as_ref()
            .map_or(ActorDisposition::Neutral, |actor| {
                ActorDisposition::from_template_value(&actor.disposition)
            }),
        inventory: Vec::new(),
        equipped_item_id: None,
    }
}

fn actor_state(actor: &ActorCapabilitiesTemplate, capabilities: &[String]) -> ActorState {
    let mut state = ActorState::configured(
        actor.hp,
        actor.damage,
        actor.can_hear,
        actor.can_see,
        actor.can_move,
        actor.combat,
    );
    state.apply_declared_capabilities(capabilities);
    state.traversal_cost = actor.traversal_cost.max(1);
    state.attack_cost = actor.attack_cost.max(1);
    state
}

fn select_objective(objectives: &[ObjectiveTemplate]) -> &ObjectiveTemplate {
    objectives
        .first()
        .expect("validated datapacks must include at least one objective")
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::{
        AbilityGrantTemplate, ActorCapabilitiesTemplate, load_datapack_bundle_by_folder,
    };

    use super::{actor_state, generate_new_run};

    #[test]
    fn generated_run_starts_in_valid_deterministic_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");

        let generated = generate_new_run(&bundle);
        let state = generated.state;

        assert_eq!(state.datapack_id, "property_siege_classic");
        assert_eq!(state.current_location_id, bundle.rules.starting_location);
        assert!(state.known_locations.contains("front_verandah"));
        assert!(state.visited_locations.contains("front_verandah"));
        assert_eq!(
            state.active_objective.target_boss_id.as_deref(),
            Some("brute_in_garage")
        );
        assert_eq!(
            state.active_objective.required_item_id.as_deref(),
            Some("house_keys")
        );
        assert_eq!(
            state.active_objective.required_location_id.as_deref(),
            Some("garage")
        );
        assert!(!state.active_objective.completed);
        assert_eq!(state.hp, 10);
        assert_eq!(state.max_hp, 10);
        assert_eq!(state.inventory.len(), 2);
        assert!(state.inventory.iter().any(|item| item.id == "torch"));
        assert!(state.inventory.iter().any(|item| item.id == "cricket_bat"));
        assert_eq!(state.equipped_item_id.as_deref(), Some("torch"));
        assert!(state.world_item_instances.values().any(|item| {
            item.effective_template_id() == "medkit"
                && item.location_id.as_deref() == Some("kitchen")
                && matches!(item.bucket, super::super::state::ItemBucket::World)
        }));
        assert!(state.enemy_instances.values().all(|instance| {
            instance.current_hp == instance.max_hp
                && matches!(instance.bucket, super::super::state::EncounterBucket::Live)
        }));
        assert!(state.boss_instances.values().all(|instance| {
            instance.current_hp == instance.max_hp
                && matches!(instance.bucket, super::super::state::EncounterBucket::Live)
        }));
        assert_eq!(
            state.active_objective.template_id,
            state.active_objective.id
        );
        assert!(matches!(
            state.active_objective.bucket,
            super::super::state::ObjectiveBucket::Active
        ));
        assert_eq!(
            state
                .location_items
                .get("kitchen")
                .cloned()
                .unwrap_or_default(),
            vec!["medkit".to_owned()]
        );
        assert_eq!(
            state
                .location_bosses
                .get("garage")
                .cloned()
                .unwrap_or_default(),
            vec!["brute_in_garage".to_owned()]
        );
        assert!(state.locked_locations.contains("garage"));
        assert!(state.locked_locations.contains("back_garden"));
        assert!(state.broken_locked_locations.is_empty());
        assert!(state.barricaded_locations.is_empty());
        assert_eq!(state.turn_index, 0);
        assert_eq!(state.noise_level, 0);
        assert_eq!(state.noise_spawn_count, 0);
        assert!(state.actor_noise_targets.is_empty());
        assert!(state.actor_origins.is_empty());
        assert!(state.actor_searching.is_empty());
        assert!(state.actor_sight_targets.is_empty());
        assert!(state.actor_sight_subjects.is_empty());
        assert!(state.actor_sight_delays.is_empty());
    }

    #[test]
    fn item_template_grants_are_owned_by_the_specific_item_instance() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        bundle.items[0].ability_grants.push(AbilityGrantTemplate {
            template_id: "grave_bolt".to_owned(),
            remaining_uses: Some(1),
            duration_turns: None,
        });

        let state = generate_new_run(&bundle).state;
        let torch = state
            .inventory
            .iter()
            .find(|item| item.id == "torch")
            .expect("starter torch");
        let grant = state
            .ability_grants
            .iter()
            .find(|grant| {
                grant.owner == super::super::state::ItemOwner::Entity(torch.instance_id.clone())
            })
            .expect("grant belongs to this exact torch instance");
        assert_eq!(grant.remaining_uses, Some(1));
        assert!(state.ability_grant_is_accessible_to(grant, &state.player_entity_id));
    }

    #[test]
    fn explicit_template_capabilities_replace_inferred_actor_payload_branches() {
        let actor = actor_state(
            &ActorCapabilitiesTemplate {
                hp: Some(8),
                damage: Some(4),
                can_hear: true,
                can_see: true,
                can_move: true,
                combat: true,
                ..Default::default()
            },
            &["inventory".to_owned()],
        );

        assert!(actor.has_capability(crate::game::capabilities::EngineCapability::Inventory));
        assert!(!actor.is_combatant());
        assert!(!actor.has_health_contract());
        assert!(!actor.has_movement_contract());
    }

    #[test]
    fn authored_scroll_grant_stays_with_its_world_item_until_taken() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let state = generate_new_run(&bundle).state;
        let scroll = state
            .world_item_instances
            .values()
            .find(|item| item.id == "grave_scroll")
            .expect("authored scroll instance");
        let grant = state
            .ability_grants
            .iter()
            .find(|grant| {
                grant.owner == super::super::state::ItemOwner::Entity(scroll.instance_id.clone())
            })
            .expect("scroll-owned grant");

        assert_eq!(grant.template_id, "spectral_spark");
        assert_eq!(grant.remaining_uses, Some(1));
        assert!(
            !state.ability_grant_is_accessible_to(grant, &state.player_entity_id),
            "the player cannot use an item grant before owning its item"
        );
    }
}
