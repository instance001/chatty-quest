use crate::data::datapacks::{
    BossTemplate, DatapackBundle, EnemyTemplate, ItemTemplate, LocationTemplate, NpcTemplate,
};

use super::actions::{ActionOutcome, GameAction, GameEvent};
use super::capabilities::{self, EngineCapability};
use super::spawned_ai::{autonomous_actor_ids, spawned_actor_can_hear};
use super::state::{
    ActorDisposition, ActorState, EncounterBucket, EncounterInstance, InstanceLifecycle,
    NpcInstance, RunState,
};

const MAX_NOISE_LEVEL: i32 = 3;

pub(super) fn apply_noise_for_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: &GameAction,
    outcome: &mut ActionOutcome,
) {
    match action {
        // Combat noise belongs to the action primitive, not to the player-shaped legacy
        // command. A requested NPC/item/enemy attack is the same host action once admitted.
        GameAction::Attack | GameAction::AttackTarget { .. } | GameAction::ActorAttack { .. } => {
            raise_noise(state, bundle, 1, outcome)
        }
        GameAction::Unlock { .. } => {
            if outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LocationUnlocked { .. }))
            {
                raise_noise(state, bundle, 1, outcome);
            }
        }
        GameAction::Barricade { .. } => {
            if outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::LocationBarricaded { .. }))
            {
                raise_noise(state, bundle, 1, outcome);
            }
        }
        _ => lower_noise(state, 1, &mut outcome.lines),
    }
}

fn raise_noise(
    state: &mut RunState,
    bundle: &DatapackBundle,
    amount: i32,
    outcome: &mut ActionOutcome,
) {
    let before = state.noise_level;
    state.noise_level = (state.noise_level + amount).clamp(0, MAX_NOISE_LEVEL);
    if state.noise_level != before {
        outcome.lines.push(format!(
            "Noise rises to {}.",
            noise_label(state.noise_level)
        ));
    }
    retarget_spawned_actors_to_noise_source(state, bundle, outcome);
    if before < MAX_NOISE_LEVEL && state.noise_level == MAX_NOISE_LEVEL {
        spawn_noise_actor(state, bundle, outcome);
    }
}

fn lower_noise(state: &mut RunState, amount: i32, lines: &mut Vec<String>) {
    let before = state.noise_level;
    state.noise_level = (state.noise_level - amount).clamp(0, MAX_NOISE_LEVEL);
    if state.noise_level != before {
        lines.push(format!(
            "Noise settles to {}.",
            noise_label(state.noise_level)
        ));
    }
}

enum NoisePoolActor<'a> {
    Enemy(&'a EnemyTemplate),
    Npc(&'a NpcTemplate),
    Boss(&'a BossTemplate),
    Item(&'a ItemTemplate),
}

impl NoisePoolActor<'_> {
    fn id(&self) -> &str {
        match self {
            Self::Enemy(template) => &template.id,
            Self::Npc(template) => &template.id,
            Self::Boss(template) => &template.id,
            Self::Item(template) => &template.id,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Enemy(template) => &template.name,
            Self::Npc(template) => &template.name,
            Self::Boss(template) => &template.name,
            Self::Item(template) => &template.name,
        }
    }
}

fn spawn_noise_actor(state: &mut RunState, bundle: &DatapackBundle, outcome: &mut ActionOutcome) {
    let Some(candidate) = select_noise_spawn_actor(state, bundle) else {
        return;
    };
    let Some(location) = select_noise_spawn_location(state, bundle) else {
        return;
    };

    let spawn_number = state.noise_spawn_count + 1;
    let actor_id = format!("noise_spawn_{}_{}", spawn_number, candidate.id());
    let template_id = candidate.id().to_owned();
    let actor_name = candidate.name().to_owned();
    state.noise_spawn_count = spawn_number;
    state
        .actor_noise_targets
        .insert(actor_id.clone(), state.current_location_id.clone());
    state
        .actor_origins
        .insert(actor_id.clone(), location.id.clone());
    state.actor_searching.remove(&actor_id);
    state.actor_sight_targets.remove(&actor_id);
    state.actor_sight_subjects.remove(&actor_id);
    state.actor_sight_delays.remove(&actor_id);

    match candidate {
        NoisePoolActor::Enemy(enemy) => {
            state.enemy_hp.insert(actor_id.clone(), enemy.hp);
            state.enemies_alive.insert(actor_id.clone());
            state.enemy_instances.insert(
                actor_id.clone(),
                EncounterInstance {
                    instance_id: actor_id.clone(),
                    template_id: enemy.id.clone(),
                    location_id: location.id.clone(),
                    bucket: EncounterBucket::Live,
                    lifecycle: InstanceLifecycle::Active,
                    current_hp: enemy.hp,
                    max_hp: enemy.hp,
                    provenance: "noise_pool".to_owned(),
                    name: enemy.name.clone(),
                    description: enemy.description.clone(),
                    damage: enemy.damage,
                    traversal_cost: enemy.traversal_cost,
                    attack_cost: enemy.attack_cost,
                    tags: enemy.tags.clone(),
                    capabilities: enemy.capabilities.clone(),
                },
            );
            state
                .location_enemies
                .entry(location.id.clone())
                .or_default()
                .push(actor_id.clone());
        }
        NoisePoolActor::Npc(npc) => {
            let mut actor = ActorState::configured(
                npc.actor.hp,
                npc.actor.damage,
                npc.actor.can_hear,
                npc.actor.can_see,
                npc.actor.can_move,
                npc.actor.combat,
            );
            actor.apply_declared_capabilities(&npc.capabilities);
            state
                .location_npcs
                .entry(location.id.clone())
                .or_default()
                .push(NpcInstance {
                    instance_id: actor_id.clone(),
                    template_id: npc.id.clone(),
                    location_id: location.id.clone(),
                    lifecycle: InstanceLifecycle::Active,
                    provenance: "noise_pool".to_owned(),
                    name: npc.name.clone(),
                    description: npc.description.clone(),
                    faction: npc.faction.clone(),
                    tags: npc.tags.clone(),
                    disposition: ActorDisposition::from_template_value(&npc.disposition),
                    actor,
                    quest_objective_ids: npc.quest_objective_ids.clone(),
                    inventory: Vec::new(),
                    equipped_item_id: None,
                });
        }
        NoisePoolActor::Boss(boss) => {
            state.boss_hp.insert(actor_id.clone(), boss.hp);
            state.bosses_alive.insert(actor_id.clone());
            state.boss_instances.insert(
                actor_id.clone(),
                EncounterInstance {
                    instance_id: actor_id.clone(),
                    template_id: boss.id.clone(),
                    location_id: location.id.clone(),
                    bucket: EncounterBucket::Live,
                    lifecycle: InstanceLifecycle::Active,
                    current_hp: boss.hp,
                    max_hp: boss.hp,
                    provenance: "noise_pool".to_owned(),
                    name: boss.name.clone(),
                    description: boss.description.clone(),
                    damage: boss.damage,
                    traversal_cost: boss.traversal_cost,
                    attack_cost: boss.attack_cost,
                    tags: boss.tags.clone(),
                    capabilities: boss.capabilities.clone(),
                },
            );
            state
                .location_bosses
                .entry(location.id.clone())
                .or_default()
                .push(actor_id.clone());
        }
        NoisePoolActor::Item(item) => {
            let actor = item
                .actor
                .as_ref()
                .expect("noise-pool item was actor-validated");
            let mut actor_state = ActorState::configured(
                actor.hp,
                actor.damage,
                actor.can_hear,
                actor.can_see,
                actor.can_move,
                actor.combat,
            );
            actor_state.apply_declared_capabilities(&item.capabilities);
            state.world_item_instances.insert(
                actor_id.clone(),
                super::state::InventoryEntry {
                    id: actor_id.clone(),
                    instance_id: actor_id.clone(),
                    template_id: item.id.clone(),
                    bucket: super::state::ItemBucket::World,
                    location_id: Some(location.id.clone()),
                    lifecycle: InstanceLifecycle::Active,
                    name: item.name.clone(),
                    description: item.description.clone(),
                    damage: item.damage,
                    origin: super::state::ItemOrigin::Authored,
                    condition: None,
                    tags: item.tags.clone(),
                    actor: Some(actor_state),
                    disposition: ActorDisposition::from_template_value(&actor.disposition),
                    inventory: Vec::new(),
                    equipped_item_id: None,
                },
            );
            state
                .location_items
                .entry(location.id.clone())
                .or_default()
                .push(actor_id.clone());
        }
    }

    outcome.events.push(GameEvent::ActorSpawnedFromNoise {
        origin_id: actor_id,
        template_id,
        location_id: location.id.clone(),
    });
    outcome.lines.push(format!(
        "Noise peaks at Swarming. {} is pulled into {}.",
        actor_name, location.name
    ));
}

fn retarget_spawned_actors_to_noise_source(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
) {
    let source_location_id = state.current_location_id.clone();
    let mut actor_ids = autonomous_actor_ids(state)
        .into_iter()
        .filter(|actor_id| spawned_actor_can_hear(state, bundle, actor_id))
        .filter(|actor_id| {
            state
                .actor_noise_targets
                .get(actor_id)
                .is_none_or(|target| target != &source_location_id)
        })
        .collect::<Vec<_>>();
    actor_ids.sort();

    if actor_ids.is_empty() {
        return;
    }

    for actor_id in &actor_ids {
        state
            .actor_noise_targets
            .insert(actor_id.clone(), source_location_id.clone());
        state.actor_searching.remove(actor_id);
        state.actor_sight_targets.remove(actor_id);
        state.actor_sight_subjects.remove(actor_id);
        state.actor_sight_delays.remove(actor_id);
    }

    outcome.events.push(GameEvent::NoiseAttractorShifted {
        location_id: source_location_id.clone(),
        actor_ids,
    });
    outcome.lines.push(format!(
        "The latest noise becomes the attractor. Spawned actors turn toward {}.",
        location_display_name_for_state_only(&source_location_id)
    ));
}

fn location_display_name_for_state_only(location_id: &str) -> String {
    location_id.replace('_', " ")
}

fn select_noise_spawn_actor<'a>(
    state: &RunState,
    bundle: &'a DatapackBundle,
) -> Option<NoisePoolActor<'a>> {
    let mut candidates = Vec::new();
    candidates.extend(bundle.enemies.iter().filter_map(|enemy| {
        (enemy.can_hear
            && (enemy.capabilities.is_empty()
                || (capabilities::contains(&enemy.capabilities, EngineCapability::Hearing)
                    && capabilities::contains(&enemy.capabilities, EngineCapability::NoiseSpawn)))
            && !noise_pool_template_is_active(state, "enemy", &enemy.id))
        .then_some(NoisePoolActor::Enemy(enemy))
    }));
    candidates.extend(bundle.npcs.iter().filter_map(|npc| {
        (npc.actor.can_hear
            && capabilities::contains(&npc.capabilities, EngineCapability::Hearing)
            && capabilities::contains(&npc.capabilities, EngineCapability::NoiseSpawn)
            && !noise_pool_template_is_active(state, "npc", &npc.id))
        .then_some(NoisePoolActor::Npc(npc))
    }));
    candidates.extend(bundle.bosses.iter().filter_map(|boss| {
        (boss.can_hear
            && capabilities::contains(&boss.capabilities, EngineCapability::Hearing)
            && capabilities::contains(&boss.capabilities, EngineCapability::NoiseSpawn)
            && !noise_pool_template_is_active(state, "boss", &boss.id))
        .then_some(NoisePoolActor::Boss(boss))
    }));
    candidates.extend(bundle.items.iter().filter_map(|item| {
        item.actor
            .as_ref()
            .filter(|actor| actor.can_hear)
            .filter(|_| capabilities::contains(&item.capabilities, EngineCapability::Hearing))
            .filter(|_| capabilities::contains(&item.capabilities, EngineCapability::NoiseSpawn))
            .filter(|_| !noise_pool_template_is_active(state, "item", &item.id))
            .map(|_| NoisePoolActor::Item(item))
    }));
    let index = deterministic_noise_index(state, candidates.len(), 17);
    candidates.into_iter().nth(index)
}

/// A noise pool may create a new runtime instance, but it must not turn one authored living
/// creature into an indistinguishable stack of same-named threats.  The same rule works for every
/// pool family and still allows an already defeated template to re-enter later if the content
/// author has another eligible trigger.
fn noise_pool_template_is_active(state: &RunState, kind: &str, template_id: &str) -> bool {
    match kind {
        "enemy" => {
            state.enemies_alive.contains(template_id)
                || state.enemy_instances.values().any(|instance| {
                    instance.template_id == template_id
                        && instance.lifecycle == InstanceLifecycle::Active
                        && instance.bucket == EncounterBucket::Live
                })
        }
        "npc" => state.location_npcs.values().flatten().any(|npc| {
            npc.template_id == template_id && npc.lifecycle == InstanceLifecycle::Active
        }),
        "boss" => {
            state.bosses_alive.contains(template_id)
                || state.boss_instances.values().any(|instance| {
                    instance.template_id == template_id
                        && instance.lifecycle == InstanceLifecycle::Active
                        && instance.bucket == EncounterBucket::Live
                })
        }
        "item" => state.world_item_instances.values().any(|item| {
            item.effective_template_id() == template_id
                && item.lifecycle == InstanceLifecycle::Active
        }),
        _ => false,
    }
}

fn select_noise_spawn_location(
    state: &RunState,
    bundle: &DatapackBundle,
) -> Option<LocationTemplate> {
    let locations = bundle
        .locations
        .iter()
        .filter(|location| {
            location.tags.iter().any(|tag| tag == "outdoor")
                || location.id.contains("yard")
                || location.id.contains("garden")
        })
        .collect::<Vec<_>>();
    if locations.is_empty() {
        return None;
    }
    let index = deterministic_noise_index(state, locations.len(), 53);
    locations.get(index).map(|location| (*location).clone())
}

pub(super) fn deterministic_noise_index(state: &RunState, len: usize, salt: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let location_score = state
        .current_location_id
        .bytes()
        .fold(0usize, |accumulator, byte| {
            accumulator.wrapping_mul(31).wrapping_add(byte as usize)
        });
    let turn_score = (state.turn_index as usize).wrapping_mul(13);
    let spawn_score = (state.noise_spawn_count as usize).wrapping_mul(37);
    location_score
        .wrapping_add(turn_score)
        .wrapping_add(spawn_score)
        .wrapping_add(salt)
        % len
}

fn noise_label(level: i32) -> &'static str {
    match level {
        0 => "Quiet",
        1 => "Stirred",
        2 => "Loud",
        _ => "Swarming",
    }
}

pub(super) fn exposed_noise_pressure_damage(state: &RunState) -> i32 {
    if state.noise_level >= 2 { 2 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::spawn_noise_actor;
    use crate::data::datapacks::{ActorCapabilitiesTemplate, load_datapack_bundle_by_folder};
    use crate::game::{ActionOutcome, generate_new_run};

    #[test]
    fn actor_capable_item_uses_the_shared_host_spawn_path() {
        let mut bundle =
            load_datapack_bundle_by_folder("property_siege_classic").expect("fixture bundle");
        bundle.enemies.clear();
        bundle.bosses.clear();
        bundle.npcs.clear();
        let item = bundle.items.first_mut().expect("fixture item");
        item.actor = Some(ActorCapabilitiesTemplate {
            hp: Some(3),
            damage: Some(1),
            can_hear: true,
            can_see: true,
            can_move: true,
            combat: true,
            traversal_cost: 1,
            attack_cost: 1,
            disposition: "hostile".to_owned(),
        });
        item.capabilities = vec![
            "health".to_owned(),
            "combat".to_owned(),
            "hearing".to_owned(),
            "sight".to_owned(),
            "noise_spawn".to_owned(),
            "noise_track".to_owned(),
            "sight_track".to_owned(),
            "movement".to_owned(),
        ];
        let mut state = generate_new_run(&bundle).state;
        let mut outcome = ActionOutcome {
            events: Vec::new(),
            lines: Vec::new(),
        };
        spawn_noise_actor(&mut state, &bundle, &mut outcome);

        let spawned = state
            .world_item_instances
            .values()
            .find(|item| item.instance_id.starts_with("noise_spawn_"))
            .expect("actor-capable item materializes as a world instance");
        assert!(
            spawned
                .actor
                .as_ref()
                .is_some_and(|actor| actor.is_combatant())
        );
        assert!(spawned.location_id.is_some());
    }
}
