use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::social::{SocialEventKind, SocialOutcome, SocialState, apply_event};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunState {
    #[serde(default = "default_player_entity_id")]
    pub player_entity_id: String,
    #[serde(default = "default_player_tags")]
    pub player_tags: Vec<String>,
    pub datapack_id: String,
    pub datapack_display_name: String,
    pub current_location_id: String,
    pub known_locations: HashSet<String>,
    pub visited_locations: HashSet<String>,
    #[serde(default)]
    pub location_instances: HashMap<String, LocationInstance>,
    #[serde(default)]
    pub temporary_exits: Vec<TemporaryExitInstance>,
    pub inventory: Vec<InventoryEntry>,
    pub equipped_item_id: Option<String>,
    #[serde(default = "default_player_media_enabled")]
    pub player_media_enabled: bool,
    #[serde(default)]
    pub status_effects: Vec<StatusEffectInstance>,
    #[serde(default)]
    pub location_npcs: HashMap<String, Vec<NpcInstance>>,
    /// Social state is keyed by the generic runtime entity id rather than by an entity noun.
    #[serde(default)]
    pub social_states: HashMap<String, SocialState>,
    /// Numeric progress is keyed by a template-defined pool (for example `necromancy`).
    /// This remains the legacy/default player's durable ledger for save compatibility.
    #[serde(default)]
    pub progress_pools: HashMap<String, i32>,
    /// Non-default runtime actors own an independent pool ledger. This avoids a controller swap
    /// making an NPC, pet, or actor-item inherit the default player's learned milestones.
    #[serde(default)]
    pub actor_progress_pools: HashMap<String, HashMap<String, i32>>,
    /// Possession is per-grant, never stored in the immutable ability template.
    #[serde(default)]
    pub ability_grants: Vec<AbilityGrant>,
    pub hp: i32,
    pub max_hp: i32,
    pub active_objective: ObjectiveState,
    #[serde(default)]
    pub additional_objectives: Vec<ObjectiveState>,
    pub enemies_alive: HashSet<String>,
    #[serde(default)]
    pub enemy_instances: HashMap<String, EncounterInstance>,
    pub enemies_defeated: HashSet<String>,
    pub enemy_hp: HashMap<String, i32>,
    pub bosses_alive: HashSet<String>,
    #[serde(default)]
    pub boss_instances: HashMap<String, EncounterInstance>,
    pub bosses_defeated: HashSet<String>,
    pub boss_hp: HashMap<String, i32>,
    pub location_items: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub world_item_instances: HashMap<String, InventoryEntry>,
    pub location_enemies: HashMap<String, Vec<String>>,
    pub location_bosses: HashMap<String, Vec<String>>,
    pub locked_locations: HashSet<String>,
    #[serde(default)]
    pub broken_locked_locations: HashSet<String>,
    pub barricaded_locations: HashSet<String>,
    #[serde(default)]
    pub turn_index: u64,
    /// Absolute readiness by runtime entity id.  `turn_index` is the authoritative world-time
    /// cursor; an actor may begin an ordinary action only once it reaches this value.  Keeping
    /// the mutable value here (rather than on a template) makes controller changes and saves
    /// preserve exactly the same timing contract.
    #[serde(default)]
    pub actor_ready_at: HashMap<String, u64>,
    /// Last turn on which the host opened a rare ambient discovery proposal form. Persisted so
    /// cooldown behavior is deterministic across save/load.
    #[serde(default)]
    pub last_ambient_proposal_turn: Option<u64>,
    pub noise_level: i32,
    #[serde(default)]
    pub noise_spawn_count: u32,
    #[serde(default, alias = "spawned_enemy_targets")]
    pub actor_noise_targets: HashMap<String, String>,
    #[serde(default, alias = "spawned_enemy_origins")]
    pub actor_origins: HashMap<String, String>,
    #[serde(default, alias = "spawned_enemy_searching")]
    pub actor_searching: HashSet<String>,
    #[serde(default, alias = "spawned_enemy_sight_targets")]
    pub actor_sight_targets: HashMap<String, String>,
    #[serde(default, alias = "spawned_enemy_sight_subjects")]
    pub actor_sight_subjects: HashMap<String, String>,
    #[serde(default, alias = "spawned_enemy_sight_delays")]
    pub actor_sight_delays: HashMap<String, u8>,
    #[serde(default)]
    pub scratch_item_admission_count: u32,
    pub boundary_response: Option<String>,
    pub rolling_summary: Vec<String>,
}

/// A read-only actor projection over canonical runtime storage. It deliberately does not become a
/// second entity store: player health remains in the compatibility fields above, while NPCs and
/// actor-items retain their existing instance buckets.
#[derive(Clone, Debug)]
pub struct RuntimeActorView {
    pub entity_id: String,
    /// Presentation metadata only; it does not choose mechanics.
    pub presentation_kind: String,
    pub template_id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub location_id: String,
    pub actor: ActorState,
    pub disposition: ActorDisposition,
}

/// Life state belongs to a materialized actor, never to its immutable template.  These tags are
/// projected alongside authored tags so normal healing and revival can share the same capability
/// handshake without mutating pack data or globally changing every instance of a template.
fn effective_actor_tags(tags: &[String], actor: &ActorState) -> Vec<String> {
    let mut effective = tags
        .iter()
        .filter(|tag| !tag.eq_ignore_ascii_case("alive") && !tag.eq_ignore_ascii_case("dead"))
        .cloned()
        .collect::<Vec<_>>();
    if actor.has_health_contract() {
        effective.push(
            if actor.current_hp.is_some_and(|hp| hp > 0) {
                "alive"
            } else {
                "dead"
            }
            .to_owned(),
        );
    }
    effective
}

fn default_player_media_enabled() -> bool {
    true
}

fn default_player_entity_id() -> String {
    "player".to_owned()
}

fn default_player_tags() -> Vec<String> {
    vec!["player".to_owned()]
}

static EMPTY_PROGRESS_POOLS: LazyLock<HashMap<String, i32>> = LazyLock::new(HashMap::new);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryEntry {
    /// Legacy item/template id retained for save compatibility and command matching.
    pub id: String,
    #[serde(default)]
    pub instance_id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub bucket: ItemBucket,
    #[serde(default)]
    pub location_id: Option<String>,
    #[serde(default)]
    pub lifecycle: InstanceLifecycle,
    pub name: String,
    pub description: String,
    pub damage: i32,
    #[serde(default)]
    pub origin: ItemOrigin,
    #[serde(default)]
    pub condition: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Present only when an item opts into the shared actor-capability path.  It has no autonomous
    /// behavior while carried; the state exists so future actor routing does not need a sentient
    /// item special case.
    #[serde(default)]
    pub actor: Option<ActorState>,
    #[serde(default)]
    pub disposition: ActorDisposition,
    /// An item can become an inventory endpoint only by opting into the host-owned inventory
    /// capability through its shared actor state. Plain items retain an empty container.
    #[serde(default)]
    pub inventory: Vec<InventoryEntry>,
    #[serde(default)]
    pub equipped_item_id: Option<String>,
}

impl InventoryEntry {
    pub fn effective_template_id(&self) -> &str {
        if self.template_id.is_empty() {
            &self.id
        } else {
            &self.template_id
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemBucket {
    #[default]
    Inventory,
    Equipped,
    World,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstanceLifecycle {
    #[default]
    Active,
    Completed,
    Consumed,
    Removed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EncounterBucket {
    #[default]
    Live,
    Defeated,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveBucket {
    #[default]
    Active,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocationInstance {
    pub instance_id: String,
    pub template_id: String,
    pub known: bool,
    pub visited: bool,
    pub locked: bool,
    #[serde(default)]
    pub lifecycle: InstanceLifecycle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporaryExitInstance {
    pub instance_id: String,
    pub template_id: String,
    pub from_location_id: String,
    pub to_location_id: String,
    pub locked: bool,
    pub lifecycle: InstanceLifecycle,
    pub provenance: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncounterInstance {
    pub instance_id: String,
    pub template_id: String,
    pub location_id: String,
    pub bucket: EncounterBucket,
    pub lifecycle: InstanceLifecycle,
    pub current_hp: i32,
    pub max_hp: i32,
    #[serde(default)]
    pub provenance: String,
    #[serde(default)]
    pub name: String,
    /// Runtime encounters keep their own copy so dynamically admitted and library entities do
    /// not lose the authored description after their proposal card closes.
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub damage: i32,
    #[serde(default = "default_action_cost")]
    pub traversal_cost: u64,
    #[serde(default = "default_action_cost")]
    pub attack_cost: u64,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The authored/proposed capability envelope captured when this encounter materialized.
    /// Empty is the compatibility representation for pre-registry encounter saves.
    #[serde(default, alias = "behavior_tags")]
    pub capabilities: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusOwner {
    #[default]
    Player,
    Entity,
    Scene,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectTrigger {
    #[default]
    Combat,
    Movement,
    Inspect,
    Wait,
    Failure,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectDuration {
    Scene,
    #[default]
    Run,
    UntilUsed,
}

/// Categories used by the common action-readiness resolver. They are deliberately action-shaped,
/// not controller- or entity-shaped, so a status can affect any valid actor uniformly.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionTimingKind {
    #[default]
    General,
    Movement,
    Combat,
    Ability,
    Wait,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusEffectInstance {
    pub instance_id: String,
    pub template_id: String,
    pub owner: StatusOwner,
    #[serde(default)]
    pub owner_id: Option<String>,
    pub trigger: EffectTrigger,
    pub duration: EffectDuration,
    pub additive_damage: i32,
    pub additive_healing: i32,
    /// Additive world-time adjustment applied when one of `timing_action_kinds` is committed.
    /// Negative values are permitted; the reducer still clamps the resolved cost to one tick.
    #[serde(default)]
    pub timing_cost_delta: i64,
    #[serde(default)]
    pub timing_action_kinds: Vec<ActionTimingKind>,
    /// A matching active status can deny an action kind before its reducer handler mutates state.
    #[serde(default)]
    pub blocked_action_kinds: Vec<ActionTimingKind>,
    #[serde(default)]
    pub required_target_tags: Vec<String>,
    #[serde(default)]
    pub required_context_tags: Vec<String>,
    #[serde(default)]
    pub lifecycle: InstanceLifecycle,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NpcInstance {
    pub instance_id: String,
    pub template_id: String,
    pub location_id: String,
    pub lifecycle: InstanceLifecycle,
    pub provenance: String,
    pub name: String,
    pub description: String,
    pub faction: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub disposition: ActorDisposition,
    #[serde(default)]
    pub actor: ActorState,
    #[serde(default)]
    pub quest_objective_ids: Vec<String>,
    #[serde(default)]
    pub inventory: Vec<InventoryEntry>,
    #[serde(default)]
    pub equipped_item_id: Option<String>,
}

/// Host-owned inventory endpoint. The controlled actor is simply one entity endpoint; nouns do
/// not create their own ownership mechanics.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ItemOwner {
    Player,
    Entity(String),
    Location(String),
}

impl ItemOwner {
    /// Canonical owner identity for event and grant projections. `Player` is retained only for
    /// save compatibility with the original top-level inventory; it names the literal default
    /// player entity, never whichever actor happens to be controlled this turn.
    pub fn entity_id(&self) -> &str {
        match self {
            Self::Player => "player",
            Self::Entity(id) | Self::Location(id) => id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AbilityGrant {
    pub instance_id: String,
    pub template_id: String,
    pub owner: ItemOwner,
    /// `None` is permanent/infinite. A number is local to this grant only.
    #[serde(default)]
    pub remaining_uses: Option<u32>,
    #[serde(default)]
    pub expires_at_turn: Option<u64>,
    /// The world turn at which this concrete grant becomes usable again. This belongs here,
    /// rather than on the immutable template, so owners and sources never share cooldown state.
    #[serde(default)]
    pub ready_at_turn: u64,
    #[serde(default)]
    pub source: AbilityGrantSource,
}

/// Why this particular owner has this mutable ability grant. The immutable ability template does
/// not carry possession, charges, or duration; those are properties of this concrete grant.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbilityGrantSource {
    #[default]
    Unlocked,
    /// A host-approved reward or admission. It is permanent unless the concrete grant has an
    /// expiry, but it did not pass through the player's XP-gated unlock path.
    Granted,
    /// Permanent knowledge obtained by consuming a concrete tagged source item through a
    /// matching literacy contract. Unlike `Unlocked`, it never spends an XP choice.
    Studied,
    Innate,
    Item,
    Temporary,
}

impl std::fmt::Display for ItemOwner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Player => formatter.write_str("player"),
            Self::Entity(instance_id) => write!(formatter, "entity:{instance_id}"),
            Self::Location(location_id) => write!(formatter, "location:{location_id}"),
        }
    }
}

fn find_item_endpoint_mut<'a>(
    entries: &'a mut [InventoryEntry],
    entity_id: &str,
) -> Option<&'a mut InventoryEntry> {
    for entry in entries {
        if entry.instance_id == entity_id {
            return Some(entry);
        }
        if let Some(found) = find_item_endpoint_mut(&mut entry.inventory, entity_id) {
            return Some(found);
        }
    }
    None
}

fn find_item_endpoint<'a>(
    entries: &'a [InventoryEntry],
    entity_id: &str,
) -> Option<&'a InventoryEntry> {
    for entry in entries {
        if entry.instance_id == entity_id {
            return Some(entry);
        }
        if let Some(found) = find_item_endpoint(&entry.inventory, entity_id) {
            return Some(found);
        }
    }
    None
}

fn inventory_contains_instance(entries: &[InventoryEntry], instance_id: &str) -> bool {
    entries.iter().any(|entry| {
        entry.instance_id == instance_id
            || inventory_contains_instance(&entry.inventory, instance_id)
    })
}

fn named_item_endpoint(entries: &[InventoryEntry], instruction: &str) -> Option<(String, String)> {
    for entry in entries {
        if instruction.contains(&entry.name.to_ascii_lowercase()) {
            return Some((entry.instance_id.clone(), entry.name.clone()));
        }
        if let Some(found) = named_item_endpoint(&entry.inventory, instruction) {
            return Some(found);
        }
    }
    None
}

fn item_endpoint_has_inventory_capability(entries: &[InventoryEntry], entity_id: &str) -> bool {
    entries.iter().any(|entry| {
        (entry.instance_id == entity_id
            && entry.actor.as_ref().is_some_and(|actor| {
                actor.has_capability(crate::game::capabilities::EngineCapability::Inventory)
            }))
            || item_endpoint_has_inventory_capability(&entry.inventory, entity_id)
    })
}

fn clear_item_endpoint_equipment(
    entries: &mut [InventoryEntry],
    entity_id: &str,
    item_id: &str,
) -> bool {
    for entry in entries {
        if entry.instance_id == entity_id {
            if entry.equipped_item_id.as_deref() == Some(item_id) {
                entry.equipped_item_id = None;
            }
            return true;
        }
        if clear_item_endpoint_equipment(&mut entry.inventory, entity_id, item_id) {
            return true;
        }
    }
    false
}

fn set_item_endpoint_equipment(
    entries: &mut [InventoryEntry],
    entity_id: &str,
    item_id: Option<String>,
) -> bool {
    for entry in entries {
        if entry.instance_id == entity_id {
            entry.equipped_item_id = item_id;
            return true;
        }
        if set_item_endpoint_equipment(&mut entry.inventory, entity_id, item_id.clone()) {
            return true;
        }
    }
    false
}

impl RunState {
    /// Returns the durable progression ledger for one actor. The literal player keeps the
    /// original top-level field so existing saves continue without migration.
    pub fn progress_pools_for_actor(&self, actor_id: &str) -> &HashMap<String, i32> {
        if actor_id == "player" {
            &self.progress_pools
        } else {
            self.actor_progress_pools
                .get(actor_id)
                .unwrap_or(&EMPTY_PROGRESS_POOLS)
        }
    }

    pub fn progress_pools_for_actor_mut(&mut self, actor_id: &str) -> &mut HashMap<String, i32> {
        if actor_id == "player" {
            &mut self.progress_pools
        } else {
            self.actor_progress_pools
                .entry(actor_id.to_owned())
                .or_default()
        }
    }
    /// The currently controlled actor's inventory endpoint. The legacy player inventory remains
    /// the endpoint for the default player entity, while any admitted actor can become the
    /// controlled endpoint without introducing a separate command path.
    pub fn controlled_inventory_owner(&self) -> ItemOwner {
        if self.player_entity_id == "player" {
            ItemOwner::Player
        } else {
            ItemOwner::Entity(self.player_entity_id.clone())
        }
    }

    /// Whether the currently controlled actor has admitted the inventory capability contract.
    /// Keeping this check beside the owner projection prevents pre-existing save data or nested
    /// item storage from accidentally making a disabled inventory branch mechanically usable.
    pub fn controlled_actor_can_carry_inventory(&self) -> bool {
        self.accepts_item_transfer(&self.controlled_inventory_owner())
    }

    /// Read-only inventory for the actor currently controlled by the player. Command adapters,
    /// gates, and presentation should use this rather than the legacy player field so they all
    /// observe the same capability-gated ownership endpoint as `TransferItem` and item reducers.
    pub fn controlled_inventory(&self) -> Option<&Vec<InventoryEntry>> {
        self.controlled_actor_can_carry_inventory()
            .then(|| self.entity_inventory(&self.controlled_inventory_owner()))
            .flatten()
    }
    /// The authoritative readiness predicate shared by player-controlled and autonomous actors.
    /// Missing entries are legacy-save compatible and mean the actor is ready at world time zero.
    pub fn actor_is_ready(&self, actor_id: &str) -> bool {
        self.turn_index >= self.actor_ready_at(actor_id)
    }

    pub fn actor_ready_at(&self, actor_id: &str) -> u64 {
        self.actor_ready_at.get(actor_id).copied().unwrap_or(0)
    }

    pub fn actor_readiness_remaining(&self, actor_id: &str) -> u64 {
        self.actor_ready_at(actor_id)
            .saturating_sub(self.turn_index)
    }

    /// Commits an actor's next ordinary readiness after a reducer-confirmed action. Costs are
    /// clamped to one tick so an accidentally authored zero cannot bypass readiness forever.
    pub fn commit_actor_action_time(&mut self, actor_id: &str, action_cost: u64) -> u64 {
        let ready_at = self.turn_index.saturating_add(action_cost.max(1));
        self.actor_ready_at.insert(actor_id.to_owned(), ready_at);
        ready_at
    }

    pub fn actor_traversal_cost(&self, actor_id: &str) -> u64 {
        self.runtime_actor(actor_id)
            .map(|actor| actor.actor.traversal_cost.max(1))
            .or_else(|| {
                self.enemy_instances
                    .get(actor_id)
                    .map(|actor| actor.traversal_cost.max(1))
            })
            .or_else(|| {
                self.boss_instances
                    .get(actor_id)
                    .map(|actor| actor.traversal_cost.max(1))
            })
            .unwrap_or(1)
    }

    pub fn actor_attack_cost(&self, actor_id: &str) -> u64 {
        self.runtime_actor(actor_id)
            .map(|actor| actor.actor.attack_cost.max(1))
            .or_else(|| {
                self.enemy_instances
                    .get(actor_id)
                    .map(|actor| actor.attack_cost.max(1))
            })
            .or_else(|| {
                self.boss_instances
                    .get(actor_id)
                    .map(|actor| actor.attack_cost.max(1))
            })
            .unwrap_or(1)
    }

    /// The literal compatibility player stores equipment in the legacy inventory fields. Expose
    /// its currently equipped damage through the same runtime actor contract as every other
    /// combatant; this is deliberately not a player-only combat action path.
    fn compatibility_player_equipped_damage(&self) -> i32 {
        self.equipped_item_id
            .as_deref()
            .and_then(|equipped_id| {
                self.inventory
                    .iter()
                    .find(|item| item.id == equipped_id || item.instance_id == equipped_id)
            })
            .map(|item| item.damage)
            .unwrap_or(1)
    }

    /// Finds a runtime item wherever the shared ownership substrate currently places it. This is
    /// deliberately an item lookup rather than an inventory-endpoint lookup: presentation,
    /// inspection, and other read-only consumers need to follow an item across player, actor,
    /// and world ownership without each rebuilding the transfer topology.
    pub fn find_runtime_item(&self, item_id: &str) -> Option<&InventoryEntry> {
        fn find_item<'a>(
            entries: &'a [InventoryEntry],
            item_id: &str,
        ) -> Option<&'a InventoryEntry> {
            for entry in entries {
                if entry.id == item_id || entry.instance_id == item_id {
                    return Some(entry);
                }
                if let Some(found) = find_item(&entry.inventory, item_id) {
                    return Some(found);
                }
            }
            None
        }

        find_item(&self.inventory, item_id)
            .or_else(|| {
                self.location_npcs
                    .values()
                    .flatten()
                    .find_map(|npc| find_item(&npc.inventory, item_id))
            })
            .or_else(|| {
                self.world_item_instances.values().find_map(|item| {
                    if item.id == item_id || item.instance_id == item_id {
                        Some(item)
                    } else {
                        find_item(&item.inventory, item_id)
                    }
                })
            })
    }

    /// Resolves an item's effective location from ownership rather than from the item's optional
    /// world-location field. A carried actor-capable item is co-located with its owner, so it can
    /// participate in the same generic actor dispatch as an item standing in the room.
    fn runtime_item_location(&self, item_id: &str) -> Option<String> {
        if inventory_contains_item(&self.inventory, item_id) {
            return Some(self.current_location_id.clone());
        }
        for npcs in self.location_npcs.values() {
            for npc in npcs {
                if inventory_contains_item(&npc.inventory, item_id) {
                    return Some(npc.location_id.clone());
                }
            }
        }
        self.world_item_instances.values().find_map(|item| {
            (matches_item(item, item_id) || inventory_contains_item(&item.inventory, item_id))
                .then(|| item.location_id.clone())
                .flatten()
        })
    }

    fn named_transfer_endpoint(&self, instruction: &str) -> Option<(String, String)> {
        self.location_npcs
            .get(&self.current_location_id)
            .into_iter()
            .flatten()
            .find(|npc| {
                npc.lifecycle == InstanceLifecycle::Active
                    && instruction.contains(&npc.name.to_ascii_lowercase())
            })
            .map(|npc| (npc.instance_id.clone(), npc.name.clone()))
            .or_else(|| {
                self.controlled_inventory()
                    .and_then(|inventory| named_item_endpoint(inventory, instruction))
            })
            .or_else(|| {
                self.location_npcs
                    .get(&self.current_location_id)
                    .into_iter()
                    .flatten()
                    .find_map(|npc| named_item_endpoint(&npc.inventory, instruction))
            })
            .or_else(|| {
                self.location_items
                    .get(&self.current_location_id)
                    .into_iter()
                    .flatten()
                    .find_map(|id| {
                        self.world_item_instances
                            .get(id)
                            .filter(|item| {
                                item.lifecycle == InstanceLifecycle::Active
                                    && instruction.contains(&item.name.to_ascii_lowercase())
                            })
                            .map(|item| (item.instance_id.clone(), item.name.clone()))
                    })
            })
    }

    pub fn resolve_player_transfer_to_actor(
        &self,
        input: &str,
    ) -> Option<Result<(String, ItemOwner, ItemOwner), String>> {
        let words = input.to_ascii_lowercase();
        let handoff_verb = ["give", "hand", "pass", "share", "offer"]
            .iter()
            .any(|verb| words.contains(verb));
        // "Marnie, take my bat" is a player-to-actor request; plain "take bat" remains the
        // normal location-to-player command and is parsed before this adapter.
        let named_take_request =
            words.contains("take") && self.named_transfer_endpoint(&words).is_some();
        if !handoff_verb && !named_take_request {
            return None;
        }
        let Some((destination_id, destination_name)) = self.named_transfer_endpoint(&words) else {
            return self
                .named_hostile_encounter(&words)
                .map(|name| Err(format!("{name} is hostile and will not accept an item.")));
        };
        if self
            .runtime_actor(&destination_id)
            .is_some_and(|actor| actor.disposition == ActorDisposition::Hostile)
        {
            return Some(Err(format!(
                "{destination_name} is hostile and will not accept an item."
            )));
        }
        if !self.entity_has_inventory_capability(&destination_id) {
            return Some(Err(format!(
                "{destination_name} has no inventory capability, so it cannot receive an item."
            )));
        }
        let controlled_owner = self.controlled_inventory_owner();
        if !self.controlled_actor_can_carry_inventory() {
            return Some(Err(
                "The controlled actor lacks the inventory capability contract.".to_owned(),
            ));
        }
        let inventory = self.controlled_inventory()?;
        Some(
            inventory
                .iter()
                .find(|item| {
                    words.contains(&item.name.to_ascii_lowercase())
                        || words.contains(&item.id.to_ascii_lowercase())
                })
                .filter(|item| item.instance_id != destination_id)
                .map(|item| {
                    (
                        item.id.clone(),
                        controlled_owner.clone(),
                        ItemOwner::Entity(destination_id),
                    )
                })
                .ok_or_else(|| format!("You do not own a matching item for {destination_name}.")),
        )
    }

    /// Resolves a concrete theft request into the same ownership endpoints used by every other
    /// transfer. This does not create an item or silently fall through to narration: a named,
    /// co-located inventory actor must actually own the requested item.
    pub fn resolve_player_theft_from_actor(
        &self,
        input: &str,
    ) -> Option<Result<(String, ItemOwner, ItemOwner), String>> {
        let words = input.to_ascii_lowercase();
        let theft_word = words
            .split(|character: char| !character.is_alphanumeric())
            .any(|word| word == "steal" || word == "theft");
        if !theft_word {
            return None;
        }
        let Some((origin_id, origin_name)) = self.named_transfer_endpoint(&words) else {
            return Some(Err(
                "Name a present inventory-capable actor and an item to steal; no ownership changed."
                    .to_owned(),
            ));
        };
        if self
            .runtime_actor(&origin_id)
            .is_some_and(|actor| actor.disposition == ActorDisposition::Hostile)
        {
            return Some(Err(format!(
                "{origin_name} is hostile and cannot be robbed through this transfer policy."
            )));
        }
        if !self.entity_has_inventory_capability(&origin_id) {
            return Some(Err(format!(
                "{origin_name} has no inventory capability, so there is no authoritative item source."
            )));
        }
        if !self.controlled_actor_can_carry_inventory() {
            return Some(Err(
                "The controlled actor lacks the inventory capability contract.".to_owned(),
            ));
        }
        let inventory = self.entity_inventory(&ItemOwner::Entity(origin_id.clone()))?;
        let controlled_owner = self.controlled_inventory_owner();
        Some(
            inventory
                .iter()
                .find(|item| {
                    words.contains(&item.name.to_ascii_lowercase())
                        || words.contains(&item.id.to_ascii_lowercase())
                })
                .map(|item| {
                    (
                        item.id.clone(),
                        ItemOwner::Entity(origin_id),
                        controlled_owner.clone(),
                    )
                })
                .ok_or_else(|| format!("{origin_name} does not own a matching item to steal.")),
        )
    }

    pub fn resolve_player_drop_to_location(
        &self,
        input: &str,
    ) -> Option<Result<(String, ItemOwner, ItemOwner), String>> {
        let words = input.to_ascii_lowercase();
        let item_name = words
            .strip_prefix("drop ")
            .or_else(|| words.strip_prefix("put down "))?
            .trim();
        if item_name.is_empty() {
            return Some(Err("Tell me what to drop.".to_owned()));
        }
        let controlled_owner = self.controlled_inventory_owner();
        if !self.controlled_actor_can_carry_inventory() {
            return Some(Err(
                "The controlled actor lacks the inventory capability contract.".to_owned(),
            ));
        }
        let inventory = self.controlled_inventory()?;
        Some(
            inventory
                .iter()
                .find(|item| {
                    crate::game::queries::matches_name(item_name, &item.id, &item.name)
                        || crate::game::queries::matches_name(
                            item_name,
                            &item.instance_id,
                            &item.name,
                        )
                })
                .map(|item| {
                    (
                        item.id.clone(),
                        controlled_owner.clone(),
                        ItemOwner::Location(self.current_location_id.clone()),
                    )
                })
                .ok_or_else(|| "You do not have that item to drop.".to_owned()),
        )
    }

    pub fn resolve_actor_transfer_to_player(
        &self,
        input: &str,
    ) -> Option<Result<(String, ItemOwner, ItemOwner), String>> {
        let words = input.to_ascii_lowercase();
        let explicit_handoff = ["give", "hand", "pass", "share"]
            .iter()
            .any(|verb| words.contains(verb));
        // Requests such as "ask the toolbox for an item" must not fall through to the narrator
        // as an invitation to invent loot.  They are a bounded ownership query: the host either
        // resolves an existing item transfer or reports why no transfer can occur.
        let generic_item_request = words.contains("ask")
            && words.contains(" for ")
            && ["item", "something", "anything", "loot"]
                .iter()
                .any(|noun| words.contains(noun));
        if !explicit_handoff && !generic_item_request {
            return None;
        }
        let Some((origin_id, origin_name)) = self.named_transfer_endpoint(&words) else {
            return self.named_hostile_encounter(&words).map(|name| {
                Err(format!(
                    "{name} is hostile and will not hand anything over."
                ))
            });
        };
        if self
            .runtime_actor(&origin_id)
            .is_some_and(|actor| actor.disposition == ActorDisposition::Hostile)
        {
            return Some(Err(format!(
                "{origin_name} is hostile and will not hand anything over."
            )));
        }
        if !self.entity_has_inventory_capability(&origin_id) {
            return Some(Err(format!(
                "{origin_name} has no inventory capability, so it cannot hand over an item."
            )));
        }
        if !self.controlled_actor_can_carry_inventory() {
            return Some(Err(
                "The controlled actor lacks the inventory capability contract.".to_owned(),
            ));
        }
        let inventory = self.entity_inventory(&ItemOwner::Entity(origin_id.clone()))?;
        let controlled_owner = self.controlled_inventory_owner();
        Some(
            inventory
                .iter()
                .find(|item| {
                    words.contains(&item.name.to_ascii_lowercase())
                        || words.contains(&item.id.to_ascii_lowercase())
                })
                .map(|item| {
                    (
                        item.id.clone(),
                        ItemOwner::Entity(origin_id),
                        controlled_owner.clone(),
                    )
                })
                .ok_or_else(|| format!("{origin_name} does not own a matching item.")),
        )
    }

    /// Returns a hostile encounter explicitly named in the player's current item request. These
    /// legacy encounter buckets are not inventory endpoints, but they are still visible entities
    /// and must not be allowed to fall through into prose-only generosity.
    fn named_hostile_encounter(&self, instruction: &str) -> Option<String> {
        self.enemy_instances
            .values()
            .chain(self.boss_instances.values())
            .filter(|encounter| {
                encounter.location_id == self.current_location_id
                    && encounter.lifecycle == InstanceLifecycle::Active
            })
            .find(|encounter| {
                let name = if encounter.name.trim().is_empty() {
                    &encounter.template_id
                } else {
                    &encounter.name
                };
                instruction.contains(&name.to_ascii_lowercase())
                    || instruction.contains(&encounter.instance_id.to_ascii_lowercase())
                    || instruction.contains(&encounter.template_id.to_ascii_lowercase())
            })
            .map(|encounter| {
                if encounter.name.trim().is_empty() {
                    encounter.template_id.clone()
                } else {
                    encounter.name.clone()
                }
            })
    }
    pub fn entity_inventory(&self, owner: &ItemOwner) -> Option<&Vec<InventoryEntry>> {
        match owner {
            ItemOwner::Player => Some(&self.inventory),
            ItemOwner::Entity(id) => {
                for npcs in self.location_npcs.values() {
                    for npc in npcs {
                        if npc.instance_id == *id {
                            return Some(&npc.inventory);
                        }
                        if let Some(item) = find_item_endpoint(&npc.inventory, id) {
                            return Some(&item.inventory);
                        }
                    }
                }
                if let Some(item) = find_item_endpoint(&self.inventory, id) {
                    return Some(&item.inventory);
                }
                for item in self.world_item_instances.values() {
                    if item.instance_id == *id {
                        return Some(&item.inventory);
                    }
                    if let Some(nested) = find_item_endpoint(&item.inventory, id) {
                        return Some(&nested.inventory);
                    }
                }
                None
            }
            ItemOwner::Location(_) => None,
        }
    }

    pub fn entity_inventory_mut(&mut self, owner: &ItemOwner) -> Option<&mut Vec<InventoryEntry>> {
        match owner {
            ItemOwner::Player => Some(&mut self.inventory),
            ItemOwner::Entity(id) => {
                for npcs in self.location_npcs.values_mut() {
                    for npc in npcs {
                        if npc.instance_id == *id {
                            return Some(&mut npc.inventory);
                        }
                        if let Some(item) = find_item_endpoint_mut(&mut npc.inventory, id) {
                            return Some(&mut item.inventory);
                        }
                    }
                }
                if let Some(item) = find_item_endpoint_mut(&mut self.inventory, id) {
                    return Some(&mut item.inventory);
                }
                for item in self.world_item_instances.values_mut() {
                    if item.instance_id == *id {
                        return Some(&mut item.inventory);
                    }
                    if let Some(nested) = find_item_endpoint_mut(&mut item.inventory, id) {
                        return Some(&mut nested.inventory);
                    }
                }
                None
            }
            ItemOwner::Location(_) => None,
        }
    }

    /// The equipped item for any inventory endpoint. This is deliberately paired with
    /// `entity_inventory` so presentation never needs to know whether its controlled actor is
    /// the legacy player, an NPC, or an actor-capable item.
    pub fn equipped_item_id_for_owner(&self, owner: &ItemOwner) -> Option<&str> {
        match owner {
            ItemOwner::Player => self.equipped_item_id.as_deref(),
            ItemOwner::Entity(id) => {
                for npcs in self.location_npcs.values() {
                    for npc in npcs {
                        if npc.instance_id == *id {
                            return npc.equipped_item_id.as_deref();
                        }
                        if let Some(item) = find_item_endpoint(&npc.inventory, id) {
                            return item.equipped_item_id.as_deref();
                        }
                    }
                }
                if let Some(item) = find_item_endpoint(&self.inventory, id) {
                    return item.equipped_item_id.as_deref();
                }
                for item in self.world_item_instances.values() {
                    if item.instance_id == *id {
                        return item.equipped_item_id.as_deref();
                    }
                    if let Some(nested) = find_item_endpoint(&item.inventory, id) {
                        return nested.equipped_item_id.as_deref();
                    }
                }
                None
            }
            ItemOwner::Location(_) => None,
        }
    }

    /// A grant owned by an item is usable by the entity currently carrying that item. This is
    /// ownership traversal, not a spell/staff special case, so nested containers stay valid.
    pub fn ability_grant_is_accessible_to(&self, grant: &AbilityGrant, origin_id: &str) -> bool {
        match &grant.owner {
            ItemOwner::Player => origin_id == "player",
            ItemOwner::Entity(owner_id) if owner_id == origin_id => true,
            ItemOwner::Entity(owner_id) => self
                .entity_inventory(&if origin_id == self.player_entity_id {
                    self.controlled_inventory_owner()
                } else {
                    ItemOwner::Entity(origin_id.to_owned())
                })
                .is_some_and(|items| inventory_contains_instance(items, owner_id)),
            ItemOwner::Location(_) => false,
        }
    }

    pub fn next_ability_grant_id(&self, template_id: &str) -> String {
        let mut index = self.ability_grants.len() + 1;
        loop {
            let candidate = format!("ability_grant_{template_id}_{index}");
            if !self
                .ability_grants
                .iter()
                .any(|grant| grant.instance_id == candidate)
            {
                return candidate;
            }
            index += 1;
        }
    }

    fn entity_has_inventory_capability(&self, entity_id: &str) -> bool {
        self.location_npcs
            .values()
            .flatten()
            .find(|npc| npc.instance_id == entity_id)
            .is_some_and(|npc| {
                npc.actor
                    .has_capability(crate::game::capabilities::EngineCapability::Inventory)
            })
            || item_endpoint_has_inventory_capability(&self.inventory, entity_id)
            || self
                .location_npcs
                .values()
                .flatten()
                .any(|npc| item_endpoint_has_inventory_capability(&npc.inventory, entity_id))
            || self.world_item_instances.values().any(|item| {
                (item.instance_id == entity_id
                    && item.actor.as_ref().is_some_and(|actor| {
                        actor.has_capability(crate::game::capabilities::EngineCapability::Inventory)
                    }))
                    || item_endpoint_has_inventory_capability(&item.inventory, entity_id)
            })
    }

    /// Whether this owner may receive an item through the one shared `TransferItem` primitive.
    /// Locations are sources/sinks for world state, while entities must explicitly opt into the
    /// host-owned inventory capability.
    pub fn accepts_item_transfer(&self, owner: &ItemOwner) -> bool {
        match owner {
            ItemOwner::Player => true,
            ItemOwner::Entity(id) => self.entity_has_inventory_capability(id),
            ItemOwner::Location(_) => false,
        }
    }

    fn clear_owner_equipment(&mut self, owner: &ItemOwner, item_id: &str) {
        match owner {
            ItemOwner::Player if self.equipped_item_id.as_deref() == Some(item_id) => {
                self.equipped_item_id = None;
            }
            ItemOwner::Entity(origin_id) => {
                if let Some(npc) = self
                    .location_npcs
                    .values_mut()
                    .flatten()
                    .find(|npc| npc.instance_id == *origin_id)
                {
                    if npc.equipped_item_id.as_deref() == Some(item_id) {
                        npc.equipped_item_id = None;
                    }
                    return;
                }
                if clear_item_endpoint_equipment(&mut self.inventory, origin_id, item_id) {
                    return;
                }
                for npcs in self.location_npcs.values_mut() {
                    for npc in npcs {
                        if clear_item_endpoint_equipment(&mut npc.inventory, origin_id, item_id) {
                            return;
                        }
                    }
                }
                for item in self.world_item_instances.values_mut() {
                    if item.instance_id == *origin_id {
                        if item.equipped_item_id.as_deref() == Some(item_id) {
                            item.equipped_item_id = None;
                        }
                        return;
                    }
                    if clear_item_endpoint_equipment(&mut item.inventory, origin_id, item_id) {
                        return;
                    }
                }
            }
            ItemOwner::Player | ItemOwner::Location(_) => {}
        }
    }

    fn set_owner_equipment(
        &mut self,
        owner: &ItemOwner,
        item_id: Option<String>,
    ) -> Result<(), String> {
        match owner {
            ItemOwner::Player => {
                self.equipped_item_id = item_id;
                Ok(())
            }
            ItemOwner::Entity(origin_id) => {
                if let Some(npc) = self
                    .location_npcs
                    .values_mut()
                    .flatten()
                    .find(|npc| npc.instance_id == *origin_id)
                {
                    npc.equipped_item_id = item_id;
                    return Ok(());
                }
                if set_item_endpoint_equipment(&mut self.inventory, origin_id, item_id.clone()) {
                    return Ok(());
                }
                for npcs in self.location_npcs.values_mut() {
                    for npc in npcs {
                        if set_item_endpoint_equipment(
                            &mut npc.inventory,
                            origin_id,
                            item_id.clone(),
                        ) {
                            return Ok(());
                        }
                    }
                }
                for item in self.world_item_instances.values_mut() {
                    if item.instance_id == *origin_id {
                        item.equipped_item_id = item_id;
                        return Ok(());
                    }
                    if set_item_endpoint_equipment(&mut item.inventory, origin_id, item_id.clone())
                    {
                        return Ok(());
                    }
                }
                Err("Owner equipment endpoint does not exist.".to_owned())
            }
            ItemOwner::Location(_) => Err("Locations cannot equip items.".to_owned()),
        }
    }

    /// Equips one item already owned by an inventory endpoint. Equipment state is held by the
    /// owner, not by a player-only compatibility field.
    pub fn equip_owned_item(
        &mut self,
        owner: &ItemOwner,
        item_id: &str,
    ) -> Result<InventoryEntry, String> {
        if !self.accepts_item_transfer(owner) {
            return Err("Owner lacks the inventory capability contract.".to_owned());
        }
        let item = self
            .entity_inventory(owner)
            .and_then(|inventory| {
                inventory
                    .iter()
                    .find(|item| item.id == item_id || item.instance_id == item_id)
            })
            .cloned()
            .ok_or_else(|| "Owner does not possess that item.".to_owned())?;
        let inventory = self
            .entity_inventory_mut(owner)
            .ok_or_else(|| "Owner inventory does not exist.".to_owned())?;
        for entry in inventory {
            entry.bucket = if entry.id == item.id || entry.instance_id == item.instance_id {
                ItemBucket::Equipped
            } else if entry.bucket == ItemBucket::Equipped {
                ItemBucket::Inventory
            } else {
                entry.bucket.clone()
            };
        }
        self.set_owner_equipment(owner, Some(item.id.clone()))?;
        Ok(item)
    }

    pub fn transfer_owned_item(
        &mut self,
        item_id: &str,
        origin: &ItemOwner,
        destination: &ItemOwner,
    ) -> Result<(), String> {
        if origin == destination {
            return Err("Origin and destination are the same owner.".to_owned());
        }
        if matches!(origin, ItemOwner::Entity(_)) && !self.accepts_item_transfer(origin) {
            return Err("Origin does not have the inventory capability.".to_owned());
        }
        if matches!(destination, ItemOwner::Entity(_)) && !self.accepts_item_transfer(destination) {
            return Err("Destination does not have the inventory capability.".to_owned());
        }
        let item = {
            let inventory = self
                .entity_inventory_mut(origin)
                .ok_or_else(|| "Origin inventory does not exist.".to_owned())?;
            let index = inventory
                .iter()
                .position(|item| item.id == item_id || item.instance_id == item_id)
                .ok_or_else(|| "Origin does not own that item.".to_owned())?;
            inventory.remove(index)
        };
        self.clear_owner_equipment(origin, &item.id);
        match destination {
            ItemOwner::Location(location_id) => {
                let mut item = item;
                item.bucket = ItemBucket::World;
                item.location_id = Some(location_id.clone());
                if item.instance_id.is_empty() {
                    item.instance_id = format!("runtime_item_{}_{}", item.id, self.turn_index);
                }
                let instance_id = item.instance_id.clone();
                self.world_item_instances.insert(instance_id.clone(), item);
                self.location_items
                    .entry(location_id.clone())
                    .or_default()
                    .push(instance_id);
            }
            ItemOwner::Player | ItemOwner::Entity(_) => self
                .entity_inventory_mut(destination)
                .ok_or_else(|| "Destination inventory does not exist.".to_owned())?
                .push(item),
        }
        Ok(())
    }

    /// Removes one concrete item from an eligible inventory endpoint. This is the shared
    /// consumption primitive for mechanics such as medicine, scroll study, or future crafting;
    /// it deliberately does not decide why the item was consumed.
    pub fn consume_owned_item(
        &mut self,
        item_id: &str,
        owner: &ItemOwner,
    ) -> Result<InventoryEntry, String> {
        if !self.accepts_item_transfer(owner) {
            return Err("Owner lacks the inventory capability contract.".to_owned());
        }
        let item = {
            let inventory = self
                .entity_inventory_mut(owner)
                .ok_or_else(|| "Owner inventory does not exist.".to_owned())?;
            let index = inventory
                .iter()
                .position(|item| item.id == item_id || item.instance_id == item_id)
                .ok_or_else(|| "Owner does not possess that item.".to_owned())?;
            inventory.remove(index)
        };
        self.clear_owner_equipment(owner, &item.id);
        Ok(item)
    }

    /// Applies a host-defined social event to a generic runtime actor.  Only this method changes
    /// relationship arithmetic or disposition; callers cannot invent per-mechanic thresholds.
    pub fn apply_social_event(
        &mut self,
        entity_id: &str,
        kind: SocialEventKind,
    ) -> Result<SocialOutcome, String> {
        let actor = self.runtime_actor(entity_id).ok_or_else(|| {
            "That entity is not an active actor with a social contract.".to_owned()
        })?;
        if actor.entity_id == self.player_entity_id {
            return Err(
                "The controlled actor does not have a relationship score toward themself."
                    .to_owned(),
            );
        }
        let outcome = {
            let state = self.social_states.entry(entity_id.to_owned()).or_default();
            apply_event(state, &actor.disposition, &actor.tags, kind)
        };
        self.set_entity_disposition(entity_id, outcome.disposition.clone())?;
        Ok(outcome)
    }

    pub fn social_state(&self, entity_id: &str) -> Option<&SocialState> {
        self.social_states.get(entity_id)
    }

    /// Bounded natural-language adapter for social acts. It identifies an existing, co-located
    /// actor and one host-known semantic event; the reducer remains the only component that
    /// changes relationship state. This is intentionally not a prose "make them friendly" hook.
    pub fn resolve_social_intent(
        &self,
        input: &str,
    ) -> Option<Result<(String, SocialEventKind), String>> {
        let words = input.to_ascii_lowercase();
        // A direct attempt to attack a named non-hostile runtime actor is a social threat until
        // the game grows a consent/assault combat policy. This does not catch "attack zombie with
        // Lira": the named actor must be the immediate target after the verb.
        for verb in ["attack", "hit"] {
            if let Some(target) = words.strip_prefix(&format!("{verb} "))
                && let Some(actor) = self
                    .runtime_actor_ids_at(&self.current_location_id)
                    .into_iter()
                    .filter(|actor_id| actor_id != &self.player_entity_id)
                    .filter_map(|actor_id| self.runtime_actor(&actor_id))
                    .find(|actor| {
                        let name = actor.name.to_ascii_lowercase();
                        target == name
                            || target.starts_with(&format!("{name} "))
                            || target.starts_with(&format!("{name},"))
                    })
            {
                return Some(Ok((actor.entity_id, SocialEventKind::Threat)));
            }
        }
        let has_word = |candidate: &str| {
            words
                .split(|character: char| !character.is_alphanumeric())
                .any(|word| word == candidate)
        };
        // Theft is an ownership claim, not a conversational state change. The command adapter
        // resolves it through the bounded transfer path before it reaches this social parser.
        if has_word("steal") || has_word("theft") {
            return Some(Err(
                "Theft needs a real item and a validated TransferItem path; no ownership changed."
                    .to_owned(),
            ));
        }
        let kind = if has_word("help") || has_word("assist") || has_word("aid") {
            SocialEventKind::Help
        } else if has_word("insult")
            || has_word("mock")
            || has_word("taunt")
            || has_word("fuck")
            || has_word("hate")
        {
            SocialEventKind::Insult
        } else if has_word("threaten") || has_word("threat") {
            SocialEventKind::Threat
        } else if has_word("betray") {
            SocialEventKind::Betrayal
        } else {
            return None;
        };
        let actor = self
            .runtime_actor_named_at(&self.current_location_id, &words)
            .ok_or_else(|| {
                "Name a present actor for that social action; no relationship state changed."
                    .to_owned()
            });
        Some(actor.map(|actor| (actor.entity_id, kind)))
    }

    fn set_entity_disposition(
        &mut self,
        entity_id: &str,
        disposition: ActorDisposition,
    ) -> Result<(), String> {
        if let Some(npc) = self
            .location_npcs
            .values_mut()
            .flatten()
            .find(|npc| npc.instance_id == entity_id)
        {
            npc.disposition = disposition;
            return Ok(());
        }
        if let Some(item) = find_item_endpoint_mut(&mut self.inventory, entity_id) {
            item.disposition = disposition;
            return Ok(());
        }
        for npcs in self.location_npcs.values_mut() {
            for npc in npcs {
                if let Some(item) = find_item_endpoint_mut(&mut npc.inventory, entity_id) {
                    item.disposition = disposition;
                    return Ok(());
                }
            }
        }
        for item in self.world_item_instances.values_mut() {
            if item.instance_id == entity_id {
                item.disposition = disposition;
                return Ok(());
            }
            if let Some(nested) = find_item_endpoint_mut(&mut item.inventory, entity_id) {
                nested.disposition = disposition;
                return Ok(());
            }
        }
        Err("That entity no longer has mutable actor state.".to_owned())
    }

    /// Restores health through the same runtime-actor substrate used by combat. The player keeps
    /// compatibility HP fields, while NPCs and actor-capable items retain their canonical actor
    /// state; callers do not need an entity-noun branch.
    pub fn restore_runtime_actor_health(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<(String, i32, i32), String> {
        if !self.runtime_actor_has_tag(entity_id, "alive") {
            return Err(
                "That actor is incapacitated; ordinary healing requires the 'alive' tag."
                    .to_owned(),
            );
        }
        self.restore_runtime_actor_health_unchecked(entity_id, amount)
    }

    /// Restores an incapacitated actor only after a reducer effect has explicitly established a
    /// `dead` target contract. This is intentionally separate from ordinary healing so a
    /// scenario's ordinary healing item can never become an accidental resurrection item.
    pub fn revive_runtime_actor_health(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<(String, i32, i32), String> {
        if !self.runtime_actor_has_tag(entity_id, "dead") {
            return Err("That actor is not incapacitated and cannot be revived.".to_owned());
        }
        self.restore_runtime_actor_health_unchecked(entity_id, amount)
    }

    fn restore_runtime_actor_health_unchecked(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<(String, i32, i32), String> {
        // `player` is the compatibility avatar. A controller may instead be assigned to any
        // materialized actor; in that case resolve that actor's own profile below rather than
        // silently projecting it as a default-speed player.
        if entity_id == self.player_entity_id && entity_id == "player" {
            let before = self.hp;
            self.hp = (self.hp + amount.max(0)).min(self.max_hp);
            return Ok(("Player".to_owned(), self.hp - before, self.hp));
        }
        if let Some(npc) =
            self.location_npcs.values_mut().flatten().find(|npc| {
                npc.instance_id == entity_id && npc.lifecycle == InstanceLifecycle::Active
            })
        {
            let (restored, current) = restore_actor_state_health(&mut npc.actor, amount)?;
            return Ok((npc.name.clone(), restored, current));
        }
        if let Some(item) = find_item_endpoint_mut(&mut self.inventory, entity_id) {
            let (restored, current) = restore_item_actor_health(item, amount)?;
            return Ok((item.name.clone(), restored, current));
        }
        for npcs in self.location_npcs.values_mut() {
            for npc in npcs {
                if let Some(item) = find_item_endpoint_mut(&mut npc.inventory, entity_id) {
                    let (restored, current) = restore_item_actor_health(item, amount)?;
                    return Ok((item.name.clone(), restored, current));
                }
            }
        }
        for item in self.world_item_instances.values_mut() {
            if item.instance_id == entity_id {
                let (restored, current) = restore_item_actor_health(item, amount)?;
                return Ok((item.name.clone(), restored, current));
            }
            if let Some(nested) = find_item_endpoint_mut(&mut item.inventory, entity_id) {
                let (restored, current) = restore_item_actor_health(nested, amount)?;
                return Ok((nested.name.clone(), restored, current));
            }
        }
        Err("That entity no longer has mutable actor health.".to_owned())
    }

    pub fn runtime_actor_has_tag(&self, entity_id: &str, tag: &str) -> bool {
        self.runtime_actor(entity_id).is_some_and(|actor| {
            actor
                .tags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(tag))
        })
    }

    /// Applies damage through the same runtime-actor surface used by combat, healing and
    /// presentation. Compatibility player fields remain authoritative only for the literal
    /// player endpoint; every materialized actor keeps its own health contract.
    pub fn damage_runtime_actor_health(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<(String, i32, i32, i32), String> {
        let amount = amount.max(0);
        if entity_id == self.player_entity_id && entity_id == "player" {
            let before = self.hp;
            self.hp = (self.hp - amount).max(0);
            return Ok(("Player".to_owned(), before - self.hp, self.hp, self.max_hp));
        }
        if let Some(npc) =
            self.location_npcs.values_mut().flatten().find(|npc| {
                npc.instance_id == entity_id && npc.lifecycle == InstanceLifecycle::Active
            })
        {
            let (applied, remaining, maximum) = damage_actor_state_health(&mut npc.actor, amount)?;
            return Ok((npc.name.clone(), applied, remaining, maximum));
        }
        if let Some(item) = find_item_endpoint_mut(&mut self.inventory, entity_id) {
            let (applied, remaining, maximum) = damage_item_actor_health(item, amount)?;
            return Ok((item.name.clone(), applied, remaining, maximum));
        }
        for npcs in self.location_npcs.values_mut() {
            for npc in npcs {
                if let Some(item) = find_item_endpoint_mut(&mut npc.inventory, entity_id) {
                    let (applied, remaining, maximum) = damage_item_actor_health(item, amount)?;
                    return Ok((item.name.clone(), applied, remaining, maximum));
                }
            }
        }
        for item in self.world_item_instances.values_mut() {
            if item.instance_id == entity_id {
                let (applied, remaining, maximum) = damage_item_actor_health(item, amount)?;
                return Ok((item.name.clone(), applied, remaining, maximum));
            }
            if let Some(nested) = find_item_endpoint_mut(&mut item.inventory, entity_id) {
                let (applied, remaining, maximum) = damage_item_actor_health(nested, amount)?;
                return Ok((nested.name.clone(), applied, remaining, maximum));
            }
        }
        if let Some(instance) = self.enemy_instances.get_mut(entity_id) {
            if !encounter_has_health_contract(instance) {
                return Err("That actor does not have a health capability.".to_owned());
            }
            // `enemy_hp` remains the save-compatible encounter ledger. Prefer it when an older
            // save or scenario adapter has updated that value, then immediately synchronize the
            // instance so every subsequent runtime-actor read sees one health truth.
            let before = self
                .enemy_hp
                .get(entity_id)
                .copied()
                .unwrap_or(instance.current_hp);
            instance.current_hp = (before - amount).max(0);
            self.enemy_hp
                .insert(entity_id.to_owned(), instance.current_hp);
            return Ok((
                instance.name.clone(),
                before - instance.current_hp,
                instance.current_hp,
                instance.max_hp,
            ));
        }
        if let Some(instance) = self.boss_instances.get_mut(entity_id) {
            if !encounter_has_health_contract(instance) {
                return Err("That actor does not have a health capability.".to_owned());
            }
            let before = self
                .boss_hp
                .get(entity_id)
                .copied()
                .unwrap_or(instance.current_hp);
            instance.current_hp = (before - amount).max(0);
            self.boss_hp
                .insert(entity_id.to_owned(), instance.current_hp);
            return Ok((
                instance.name.clone(),
                before - instance.current_hp,
                instance.current_hp,
                instance.max_hp,
            ));
        }
        Err("That entity no longer has mutable actor health.".to_owned())
    }

    /// Raises the maximum health of the identified runtime actor without healing it. Progression
    /// and future durable effects use this rather than assuming the compatibility player owns
    /// every health pool.
    pub fn increase_runtime_actor_max_health(
        &mut self,
        entity_id: &str,
        amount: i32,
    ) -> Result<(String, i32), String> {
        let amount = amount.max(0);
        if entity_id == self.player_entity_id && entity_id == "player" {
            self.max_hp = self.max_hp.saturating_add(amount);
            return Ok(("Player".to_owned(), self.max_hp));
        }
        if let Some(npc) =
            self.location_npcs.values_mut().flatten().find(|npc| {
                npc.instance_id == entity_id && npc.lifecycle == InstanceLifecycle::Active
            })
        {
            let maximum = increase_actor_state_max_health(&mut npc.actor, amount)?;
            return Ok((npc.name.clone(), maximum));
        }
        if let Some(item) = find_item_endpoint_mut(&mut self.inventory, entity_id) {
            let maximum = increase_item_actor_max_health(item, amount)?;
            return Ok((item.name.clone(), maximum));
        }
        for npcs in self.location_npcs.values_mut() {
            for npc in npcs {
                if let Some(item) = find_item_endpoint_mut(&mut npc.inventory, entity_id) {
                    let maximum = increase_item_actor_max_health(item, amount)?;
                    return Ok((item.name.clone(), maximum));
                }
            }
        }
        for item in self.world_item_instances.values_mut() {
            if item.instance_id == entity_id {
                let maximum = increase_item_actor_max_health(item, amount)?;
                return Ok((item.name.clone(), maximum));
            }
            if let Some(nested) = find_item_endpoint_mut(&mut item.inventory, entity_id) {
                let maximum = increase_item_actor_max_health(nested, amount)?;
                return Ok((nested.name.clone(), maximum));
            }
        }
        if let Some(instance) = self.enemy_instances.get_mut(entity_id) {
            if !encounter_has_health_contract(instance) {
                return Err("That actor does not have a health capability.".to_owned());
            }
            instance.max_hp = instance.max_hp.saturating_add(amount);
            return Ok((instance.name.clone(), instance.max_hp));
        }
        if let Some(instance) = self.boss_instances.get_mut(entity_id) {
            if !encounter_has_health_contract(instance) {
                return Err("That actor does not have a health capability.".to_owned());
            }
            instance.max_hp = instance.max_hp.saturating_add(amount);
            return Ok((instance.name.clone(), instance.max_hp));
        }
        Err("That entity no longer has mutable actor health.".to_owned())
    }

    /// Healthless controllers remain valid for non-health mechanics; an actor with an explicit
    /// health contract is incapacitated when its HP reaches zero.
    pub fn controlled_actor_is_alive(&self) -> bool {
        self.runtime_actor(&self.player_entity_id)
            .filter(|actor| actor.actor.has_health_contract())
            .and_then(|actor| actor.actor.current_hp)
            .is_none_or(|hp| hp > 0)
    }

    /// Health for the current controller, with legacy player fields retained only as a
    /// compatibility fallback for healthless or old-save actors.
    pub fn controlled_actor_health(&self) -> (i32, i32) {
        self.runtime_actor(&self.player_entity_id)
            .filter(|actor| actor.actor.has_health_contract())
            .and_then(|actor| actor.actor.current_hp.zip(actor.actor.max_hp))
            .unwrap_or((self.hp, self.max_hp))
    }
}

fn restore_actor_state_health(actor: &mut ActorState, amount: i32) -> Result<(i32, i32), String> {
    if !actor.has_health_contract() {
        return Err("That actor does not have a health capability.".to_owned());
    }
    let Some(current) = actor.current_hp else {
        return Err("That actor does not have a health capability.".to_owned());
    };
    let Some(maximum) = actor.max_hp else {
        return Err("That actor has no maximum health contract.".to_owned());
    };
    let updated = (current + amount.max(0)).min(maximum);
    actor.current_hp = Some(updated);
    Ok((updated - current, updated))
}

fn damage_actor_state_health(
    actor: &mut ActorState,
    amount: i32,
) -> Result<(i32, i32, i32), String> {
    if !actor.has_health_contract() {
        return Err("That actor does not have a health capability.".to_owned());
    }
    let Some(current) = actor.current_hp else {
        return Err("That actor does not have a health capability.".to_owned());
    };
    let Some(maximum) = actor.max_hp else {
        return Err("That actor has no maximum health contract.".to_owned());
    };
    let updated = (current - amount.max(0)).max(0);
    actor.current_hp = Some(updated);
    Ok((current - updated, updated, maximum))
}

fn increase_actor_state_max_health(actor: &mut ActorState, amount: i32) -> Result<i32, String> {
    if !actor.has_health_contract() {
        return Err("That actor does not have a health capability.".to_owned());
    }
    let Some(maximum) = actor.max_hp else {
        return Err("That actor has no maximum health contract.".to_owned());
    };
    actor.max_hp = Some(maximum.saturating_add(amount.max(0)));
    Ok(actor.max_hp.expect("assigned maximum health"))
}

fn restore_item_actor_health(item: &mut InventoryEntry, amount: i32) -> Result<(i32, i32), String> {
    if item.lifecycle != InstanceLifecycle::Active {
        return Err("That actor item is no longer active.".to_owned());
    }
    let Some(actor) = item.actor.as_mut() else {
        return Err("That item does not have an actor health contract.".to_owned());
    };
    restore_actor_state_health(actor, amount)
}

fn damage_item_actor_health(
    item: &mut InventoryEntry,
    amount: i32,
) -> Result<(i32, i32, i32), String> {
    if item.lifecycle != InstanceLifecycle::Active {
        return Err("That actor item is no longer active.".to_owned());
    }
    let Some(actor) = item.actor.as_mut() else {
        return Err("That item is not an actor with a health capability.".to_owned());
    };
    damage_actor_state_health(actor, amount)
}

fn increase_item_actor_max_health(item: &mut InventoryEntry, amount: i32) -> Result<i32, String> {
    if item.lifecycle != InstanceLifecycle::Active {
        return Err("That actor item is no longer active.".to_owned());
    }
    let Some(actor) = item.actor.as_mut() else {
        return Err("That item is not an actor with a health capability.".to_owned());
    };
    increase_actor_state_max_health(actor, amount)
}

/// Shared runtime state for any entity type that opts into actor capabilities.  Existing enemy and
/// boss encounter instances keep their legacy storage for save compatibility; NPCs and optional
/// actor-capable items use this same state rather than inventing type-specific copies.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ActorState {
    #[serde(default)]
    pub current_hp: Option<i32>,
    #[serde(default)]
    pub max_hp: Option<i32>,
    #[serde(default)]
    pub damage: Option<i32>,
    #[serde(default)]
    pub can_hear: bool,
    #[serde(default)]
    pub can_see: bool,
    #[serde(default)]
    pub can_move: bool,
    #[serde(default)]
    pub combat: bool,
    /// Integer world-time cost for a normal traversal.  One preserves legacy turn pacing.
    #[serde(default = "default_action_cost")]
    pub traversal_cost: u64,
    /// Integer world-time cost for this actor's unarmed/native combat action.
    #[serde(default = "default_action_cost")]
    pub attack_cost: u64,
    /// Host-owned capabilities copied from the selected template at materialization time. They
    /// are deliberately separate from descriptive/media tags.
    #[serde(default, alias = "behavior_tags")]
    pub capabilities: Vec<String>,
}

impl ActorState {
    pub fn configured(
        hp: Option<i32>,
        damage: Option<i32>,
        can_hear: bool,
        can_see: bool,
        can_move: bool,
        combat: bool,
    ) -> Self {
        let mut capabilities = Vec::new();
        if hp.is_some() {
            capabilities.push("health".to_owned());
        }
        if can_hear {
            capabilities.push("hearing".to_owned());
        }
        if can_see {
            capabilities.push("sight".to_owned());
        }
        if can_move {
            capabilities.push("movement".to_owned());
        }
        if combat {
            capabilities.push("combat".to_owned());
        }
        Self {
            current_hp: hp,
            max_hp: hp,
            damage,
            can_hear,
            can_see,
            can_move,
            combat,
            traversal_cost: default_action_cost(),
            attack_cost: default_action_cost(),
            capabilities,
        }
    }

    pub fn is_combatant(&self) -> bool {
        // `combat` and the scalar fields are retained for legacy-template compatibility, but an
        // explicitly populated capability list is authoritative. This makes a live capability
        // transition (combat removed while old damage remains stored) actually revoke the
        // reducer action instead of leaving a decorative checkbox behind. Empty lists are old
        // save/template data and retain their historical inferred contract.
        self.combat
            && self.has_health_contract()
            && self.damage.is_some()
            && (self.capabilities.is_empty()
                || self.has_capability(crate::game::capabilities::EngineCapability::Combat))
    }

    /// Health is a capability-owned contract, not merely two legacy scalar fields.  Empty
    /// capability lists are pre-registry save/template data and retain their inferred contract;
    /// once a host has selected an explicit capability envelope, removing `health` makes stale
    /// HP values inapplicable to all reducer paths.
    pub fn has_health_contract(&self) -> bool {
        self.current_hp.is_some()
            && self.max_hp.is_some()
            && (self.capabilities.is_empty()
                || self.has_capability(crate::game::capabilities::EngineCapability::Health))
    }

    /// Sense and traversal booleans are legacy payload fields.  Once an actor has an explicit
    /// capability envelope, the matching root keeps them mechanically honest just like health
    /// and combat do.  Empty envelopes remain compatible with older authored/save data.
    pub fn has_hearing_contract(&self) -> bool {
        self.can_hear
            && (self.capabilities.is_empty()
                || self.has_capability(crate::game::capabilities::EngineCapability::Hearing))
    }

    pub fn has_sight_contract(&self) -> bool {
        self.can_see
            && (self.capabilities.is_empty()
                || self.has_capability(crate::game::capabilities::EngineCapability::Sight))
    }

    pub fn has_movement_contract(&self) -> bool {
        self.can_move
            && (self.capabilities.is_empty()
                || self.has_capability(crate::game::capabilities::EngineCapability::Movement))
    }

    pub fn has_capability(&self, capability: crate::game::capabilities::EngineCapability) -> bool {
        crate::game::capabilities::contains(&self.capabilities, capability)
    }

    pub fn add_declared_capabilities(&mut self, declared: &[String]) {
        for capability in declared {
            let Some(capability) = crate::game::capabilities::EngineCapability::parse(capability)
            else {
                continue;
            };
            let id = capability.id().to_owned();
            if !self.capabilities.iter().any(|candidate| candidate == &id) {
                self.capabilities.push(id);
            }
        }
    }

    /// Applies an authored/proposed capability envelope at materialization time.  A non-empty
    /// declaration is authoritative: legacy scalar fields may supply values for selected roots,
    /// but they must not silently add roots the author or approval card omitted.  An empty list
    /// intentionally retains `configured`'s inferred legacy contract for old content and saves.
    pub fn apply_declared_capabilities(&mut self, declared: &[String]) {
        if declared.is_empty() {
            return;
        }
        self.capabilities.clear();
        self.add_declared_capabilities(declared);
    }
}

pub const fn default_action_cost() -> u64 {
    1
}

/// Encounter templates historically had implicit health/combat/sense behavior.  Preserve that
/// only for an empty legacy envelope; a new explicit selection is copied verbatim into the
/// runtime actor view and therefore gates every reducer path through the normal contract checks.
fn encounter_capabilities(capabilities: &[String]) -> Vec<String> {
    if capabilities.is_empty() {
        vec![
            "health".to_owned(),
            "hearing".to_owned(),
            "sight".to_owned(),
            "movement".to_owned(),
            "combat".to_owned(),
        ]
    } else {
        capabilities.to_vec()
    }
}

/// Encounter health remains inferred only for records from before capability envelopes were
/// persisted. For every explicit envelope, stale HP scalars are inert unless the health root is
/// selected.
fn encounter_has_health_contract(encounter: &EncounterInstance) -> bool {
    encounter.capabilities.is_empty()
        || crate::game::capabilities::contains(
            &encounter.capabilities,
            crate::game::capabilities::EngineCapability::Health,
        )
}

impl RunState {
    /// Resolves the controlled actor, an NPC, or a world item through one actor-shaped read
    /// surface. The literal player projection is derived from the authoritative compatibility
    /// state so old saves retain a single HP/inventory authority during the staged migration.
    pub fn runtime_actor(&self, entity_id: &str) -> Option<RuntimeActorView> {
        // Only the literal compatibility avatar uses the legacy player fields. If control moves
        // to a materialized encounter, continue into the shared entity projections below.
        if entity_id == self.player_entity_id && entity_id == "player" {
            let actor = ActorState {
                current_hp: Some(self.hp),
                max_hp: Some(self.max_hp),
                damage: Some(self.compatibility_player_equipped_damage()),
                can_hear: true,
                can_see: true,
                can_move: true,
                combat: true,
                traversal_cost: default_action_cost(),
                attack_cost: default_action_cost(),
                capabilities: vec![
                    "health".to_owned(),
                    "hearing".to_owned(),
                    "sight".to_owned(),
                    "movement".to_owned(),
                    "combat".to_owned(),
                    "inventory".to_owned(),
                ],
            };
            return Some(RuntimeActorView {
                entity_id: self.player_entity_id.clone(),
                presentation_kind: "Player".to_owned(),
                template_id: "player".to_owned(),
                name: "Player".to_owned(),
                description: "The controlled actor character.".to_owned(),
                tags: effective_actor_tags(&self.player_tags, &actor),
                location_id: self.current_location_id.clone(),
                actor,
                disposition: ActorDisposition::Neutral,
            });
        }
        if self.enemies_alive.contains(entity_id)
            && let Some(encounter) = self.enemy_instances.get(entity_id)
            && encounter.lifecycle == InstanceLifecycle::Active
        {
            return Some(RuntimeActorView {
                entity_id: entity_id.to_owned(),
                presentation_kind: "Enemy".to_owned(),
                template_id: encounter.template_id.clone(),
                name: encounter.name.clone(),
                description: encounter.description.clone(),
                tags: effective_actor_tags(
                    &encounter.tags,
                    &ActorState {
                        current_hp: Some(encounter.current_hp),
                        max_hp: Some(encounter.max_hp),
                        capabilities: encounter_capabilities(&encounter.capabilities),
                        ..Default::default()
                    },
                ),
                location_id: encounter.location_id.clone(),
                actor: ActorState {
                    current_hp: Some(encounter.current_hp),
                    max_hp: Some(encounter.max_hp),
                    damage: Some(encounter.damage),
                    can_hear: true,
                    can_see: true,
                    can_move: true,
                    combat: true,
                    traversal_cost: encounter.traversal_cost.max(1),
                    attack_cost: encounter.attack_cost.max(1),
                    capabilities: encounter_capabilities(&encounter.capabilities),
                },
                disposition: ActorDisposition::Hostile,
            });
        }
        if self.bosses_alive.contains(entity_id)
            && let Some(encounter) = self.boss_instances.get(entity_id)
            && encounter.lifecycle == InstanceLifecycle::Active
        {
            return Some(RuntimeActorView {
                entity_id: entity_id.to_owned(),
                presentation_kind: "Boss".to_owned(),
                template_id: encounter.template_id.clone(),
                name: encounter.name.clone(),
                description: encounter.description.clone(),
                tags: effective_actor_tags(
                    &encounter.tags,
                    &ActorState {
                        current_hp: Some(encounter.current_hp),
                        max_hp: Some(encounter.max_hp),
                        capabilities: encounter_capabilities(&encounter.capabilities),
                        ..Default::default()
                    },
                ),
                location_id: encounter.location_id.clone(),
                actor: ActorState {
                    current_hp: Some(encounter.current_hp),
                    max_hp: Some(encounter.max_hp),
                    damage: Some(encounter.damage),
                    can_hear: true,
                    can_see: true,
                    can_move: true,
                    combat: true,
                    traversal_cost: encounter.traversal_cost.max(1),
                    attack_cost: encounter.attack_cost.max(1),
                    capabilities: encounter_capabilities(&encounter.capabilities),
                },
                disposition: ActorDisposition::Hostile,
            });
        }
        if let Some(npc) =
            self.location_npcs.values().flatten().find(|npc| {
                npc.instance_id == entity_id && npc.lifecycle == InstanceLifecycle::Active
            })
        {
            return Some(RuntimeActorView {
                entity_id: npc.instance_id.clone(),
                presentation_kind: "NPC".to_owned(),
                template_id: npc.template_id.clone(),
                name: npc.name.clone(),
                description: npc.description.clone(),
                tags: effective_actor_tags(&npc.tags, &npc.actor),
                location_id: npc.location_id.clone(),
                actor: npc.actor.clone(),
                disposition: npc.disposition.clone(),
            });
        }
        self.find_runtime_item(entity_id).and_then(|item| {
            let actor = item.actor.clone()?;
            let location_id = self.runtime_item_location(entity_id)?;
            (item.lifecycle == InstanceLifecycle::Active).then(|| RuntimeActorView {
                entity_id: if item.instance_id.is_empty() {
                    item.id.clone()
                } else {
                    item.instance_id.clone()
                },
                presentation_kind: "Actor item".to_owned(),
                template_id: item.effective_template_id().to_owned(),
                name: item.name.clone(),
                description: item.description.clone(),
                tags: effective_actor_tags(&item.tags, &actor),
                location_id,
                actor,
                disposition: item.disposition.clone(),
            })
        })
    }

    /// Returns the active, actor-capable entities at a location through one stable directory.
    /// Storage buckets remain a compatibility detail; callers should select behaviour from the
    /// returned capability contract and disposition rather than from the entity noun.
    pub fn runtime_actor_ids_at(&self, location_id: &str) -> Vec<String> {
        let mut ids = Vec::new();
        if self.current_location_id == location_id {
            ids.push(self.player_entity_id.clone());
        }
        ids.extend(
            self.location_npcs
                .get(location_id)
                .into_iter()
                .flatten()
                .filter(|npc| npc.lifecycle == InstanceLifecycle::Active)
                .map(|npc| npc.instance_id.clone()),
        );
        ids.extend(
            self.location_enemies
                .get(location_id)
                .into_iter()
                .flatten()
                .filter(|id| self.enemies_alive.contains(*id))
                .cloned(),
        );
        ids.extend(
            self.location_bosses
                .get(location_id)
                .into_iter()
                .flatten()
                .filter(|id| self.bosses_alive.contains(*id))
                .cloned(),
        );
        if self.current_location_id == location_id {
            collect_actor_item_ids(&self.inventory, &mut ids);
        }
        for npc in self.location_npcs.values().flatten() {
            if npc.location_id == location_id && npc.lifecycle == InstanceLifecycle::Active {
                collect_actor_item_ids(&npc.inventory, &mut ids);
            }
        }
        for item in self.world_item_instances.values() {
            if item.location_id.as_deref() == Some(location_id) {
                collect_actor_item_ids(std::slice::from_ref(item), &mut ids);
            }
        }
        ids.sort();
        ids.dedup();
        ids
    }

    /// Resolves a named non-player actor from the active location directory. Natural-language
    /// adapters use this only to identify an existing entity; action validation remains reducer
    /// owned.
    pub fn runtime_actor_named_at(
        &self,
        location_id: &str,
        normalized_instruction: &str,
    ) -> Option<RuntimeActorView> {
        self.runtime_actor_ids_at(location_id)
            .into_iter()
            .filter(|actor_id| actor_id != &self.player_entity_id)
            .filter_map(|actor_id| self.runtime_actor(&actor_id))
            .find(|actor| normalized_instruction.contains(&actor.name.to_ascii_lowercase()))
    }
}

fn matches_item(item: &InventoryEntry, item_id: &str) -> bool {
    item.id == item_id || item.instance_id == item_id
}

fn inventory_contains_item(entries: &[InventoryEntry], item_id: &str) -> bool {
    entries.iter().any(|entry| {
        matches_item(entry, item_id) || inventory_contains_item(&entry.inventory, item_id)
    })
}

fn collect_actor_item_ids(entries: &[InventoryEntry], ids: &mut Vec<String>) {
    for item in entries {
        if item.lifecycle == InstanceLifecycle::Active && item.actor.is_some() {
            ids.push(if item.instance_id.is_empty() {
                item.id.clone()
            } else {
                item.instance_id.clone()
            });
        }
        collect_actor_item_ids(&item.inventory, ids);
    }
}

impl ActorDisposition {
    pub fn from_template_value(value: &str) -> Self {
        match value.trim() {
            "friendly" => Self::Friendly,
            "hostile" => Self::Hostile,
            _ => Self::Neutral,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorDisposition {
    Friendly,
    #[default]
    Neutral,
    Hostile,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ItemOrigin {
    #[default]
    Authored,
    NarratorApproved,
}

#[cfg(test)]
mod tests {
    use super::{
        ActorDisposition, ActorState, InstanceLifecycle, ItemOwner, NpcInstance, RunState,
    };

    #[test]
    fn controlled_player_resolves_through_the_shared_runtime_actor_view() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;

        let player = run
            .runtime_actor(&run.player_entity_id)
            .expect("controlled player resolves as a runtime actor");

        assert_eq!(player.entity_id, run.player_entity_id);
        assert_eq!(player.location_id, run.current_location_id);
        assert_eq!(player.actor.current_hp, Some(run.hp));
        assert_eq!(player.actor.max_hp, Some(run.max_hp));
        assert_eq!(player.actor.damage, Some(1));
        assert!(player.actor.combat);
        assert!(player.actor.is_combatant());
        assert!(
            player
                .actor
                .has_capability(crate::game::capabilities::EngineCapability::Inventory)
        );
    }

    #[test]
    fn player_runtime_combat_damage_tracks_the_equipped_item() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let bat_id = run
            .inventory
            .iter()
            .find(|item| item.name == "Battered Cricket Bat")
            .expect("fixture bat")
            .id
            .clone();
        run.equipped_item_id = Some(bat_id);

        assert_eq!(
            run.runtime_actor("player")
                .and_then(|actor| actor.actor.damage),
            Some(3)
        );
    }

    #[test]
    fn explicit_capability_selection_can_revoke_a_stale_combat_payload() {
        let actor = ActorState {
            current_hp: Some(10),
            max_hp: Some(10),
            damage: Some(3),
            combat: true,
            // A non-empty contract says this is deliberately an inventory-only actor; old
            // scalar combat data must not keep its action path alive.
            capabilities: vec!["inventory".to_owned()],
            ..Default::default()
        };

        assert!(!actor.is_combatant());
    }

    #[test]
    fn explicit_capability_selection_can_revoke_a_stale_health_payload() {
        let actor = ActorState {
            current_hp: Some(10),
            max_hp: Some(10),
            // A non-empty contract says this is deliberately a sight-only actor; old HP
            // fields must not continue to expose health mechanics.
            capabilities: vec!["sight".to_owned()],
            ..Default::default()
        };

        assert!(!actor.has_health_contract());
    }

    #[test]
    fn explicit_capability_selection_gates_stale_senses_and_movement() {
        let actor = ActorState {
            can_hear: true,
            can_see: true,
            can_move: true,
            capabilities: vec!["inventory".to_owned()],
            ..Default::default()
        };

        assert!(!actor.has_hearing_contract());
        assert!(!actor.has_sight_contract());
        assert!(!actor.has_movement_contract());
    }

    #[test]
    fn explicit_materialization_replaces_inferred_legacy_capabilities() {
        let mut actor = ActorState::configured(Some(10), Some(3), true, true, true, true);
        actor.apply_declared_capabilities(&["inventory".to_owned()]);

        assert!(actor.has_capability(crate::game::capabilities::EngineCapability::Inventory));
        assert!(!actor.has_health_contract());
        assert!(!actor.is_combatant());
        assert!(!actor.has_hearing_contract());
        assert!(!actor.has_sight_contract());
        assert!(!actor.has_movement_contract());
    }

    #[test]
    fn explicit_encounter_capabilities_gate_the_runtime_actor_view() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let encounter_id = run
            .enemy_instances
            .keys()
            .next()
            .expect("fixture enemy")
            .clone();
        run.enemy_instances
            .get_mut(&encounter_id)
            .expect("fixture encounter")
            .capabilities = vec!["health".to_owned()];

        let actor = run
            .runtime_actor(&encounter_id)
            .expect("live encounter resolves as an actor");
        assert!(actor.actor.has_health_contract());
        assert!(!actor.actor.is_combatant());
    }

    #[test]
    fn health_mutation_api_cannot_bypass_an_explicit_encounter_envelope() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let encounter_id = run
            .enemy_instances
            .keys()
            .next()
            .expect("fixture enemy")
            .clone();
        let hp_before = run.enemy_hp[&encounter_id];
        run.enemy_instances
            .get_mut(&encounter_id)
            .expect("fixture encounter")
            .capabilities = vec!["combat".to_owned()];

        assert!(run.damage_runtime_actor_health(&encounter_id, 3).is_err());
        assert!(
            run.increase_runtime_actor_max_health(&encounter_id, 3)
                .is_err()
        );
        assert_eq!(run.enemy_hp[&encounter_id], hp_before);
    }

    #[test]
    fn encounter_health_mutation_reconciles_legacy_ledger_into_runtime_instance() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let encounter_id = run
            .enemy_instances
            .keys()
            .next()
            .expect("fixture enemy")
            .clone();
        run.enemy_hp.insert(encounter_id.clone(), 2);

        run.damage_runtime_actor_health(&encounter_id, 1)
            .expect("health contract");

        assert_eq!(run.enemy_hp[&encounter_id], 1);
        assert_eq!(
            run.enemy_instances[&encounter_id].current_hp, 1,
            "the instance must converge on the compatibility ledger"
        );
    }

    #[test]
    fn older_run_saves_default_new_instance_buckets_to_empty() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;
        let mut value = serde_json::to_value(run).expect("run serializes");
        let object = value.as_object_mut().expect("run uses an object schema");
        object.remove("location_npcs");
        object.remove("additional_objectives");
        object.remove("temporary_exits");
        if let Some(instances) = object
            .get_mut("enemy_instances")
            .and_then(serde_json::Value::as_object_mut)
            && let Some(instance) = instances
                .values_mut()
                .next()
                .and_then(serde_json::Value::as_object_mut)
        {
            instance.remove("name");
            instance.remove("damage");
            instance.remove("tags");
        }
        let restored: RunState = serde_json::from_value(value).expect("old run still loads");
        assert!(restored.location_npcs.is_empty());
        assert!(restored.additional_objectives.is_empty());
        assert!(restored.temporary_exits.is_empty());
        assert!(restored.enemy_instances.values().any(|instance| {
            instance.name.is_empty() && instance.damage == 0 && instance.tags.is_empty()
        }));
    }

    #[test]
    fn actor_readiness_round_trips_and_missing_legacy_field_means_ready_now() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        run.turn_index = 4;
        run.actor_ready_at.insert("player".to_owned(), 9);
        let restored: RunState =
            serde_json::from_value(serde_json::to_value(&run).unwrap()).unwrap();
        assert_eq!(restored.actor_ready_at("player"), 9);
        assert!(!restored.actor_is_ready("player"));

        let mut legacy = serde_json::to_value(run).unwrap();
        legacy
            .as_object_mut()
            .expect("run object")
            .remove("actor_ready_at");
        let restored_legacy: RunState = serde_json::from_value(legacy).unwrap();
        assert!(restored_legacy.actor_is_ready("player"));
    }

    #[test]
    fn legacy_enemy_named_autonomy_blackboard_loads_into_actor_neutral_fields() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;
        let mut value = serde_json::to_value(run).expect("run serializes");
        let object = value.as_object_mut().expect("run uses an object schema");
        for field in [
            "actor_noise_targets",
            "actor_origins",
            "actor_searching",
            "actor_sight_targets",
            "actor_sight_subjects",
            "actor_sight_delays",
        ] {
            object.remove(field);
        }
        object.insert(
            "spawned_enemy_targets".to_owned(),
            serde_json::json!({ "legacy_actor": "kitchen" }),
        );
        object.insert(
            "spawned_enemy_origins".to_owned(),
            serde_json::json!({ "legacy_actor": "front_verandah" }),
        );
        object.insert(
            "spawned_enemy_searching".to_owned(),
            serde_json::json!(["legacy_actor"]),
        );
        object.insert(
            "spawned_enemy_sight_targets".to_owned(),
            serde_json::json!({ "legacy_actor": "kitchen" }),
        );
        object.insert(
            "spawned_enemy_sight_subjects".to_owned(),
            serde_json::json!({ "legacy_actor": "player" }),
        );
        object.insert(
            "spawned_enemy_sight_delays".to_owned(),
            serde_json::json!({ "legacy_actor": 1 }),
        );

        let restored: RunState = serde_json::from_value(value).expect("legacy run still loads");
        assert_eq!(
            restored.actor_noise_targets.get("legacy_actor"),
            Some(&"kitchen".to_owned())
        );
        assert_eq!(
            restored.actor_origins.get("legacy_actor"),
            Some(&"front_verandah".to_owned())
        );
        assert!(restored.actor_searching.contains("legacy_actor"));
        assert_eq!(
            restored.actor_sight_targets.get("legacy_actor"),
            Some(&"kitchen".to_owned())
        );
        assert_eq!(
            restored.actor_sight_subjects.get("legacy_actor"),
            Some(&"player".to_owned())
        );
        assert_eq!(restored.actor_sight_delays.get("legacy_actor"), Some(&1));
    }

    #[test]
    fn player_media_preference_persists_and_old_runs_default_to_enabled() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        run.player_media_enabled = false;
        let restored: RunState =
            serde_json::from_value(serde_json::to_value(&run).expect("run serializes"))
                .expect("run restores");
        assert!(!restored.player_media_enabled);

        let mut legacy = serde_json::to_value(run).expect("run serializes");
        legacy
            .as_object_mut()
            .expect("run is an object")
            .remove("player_media_enabled");
        let restored: RunState = serde_json::from_value(legacy).expect("legacy run restores");
        assert!(restored.player_media_enabled);
    }

    #[test]
    fn actor_capable_item_state_is_opt_in_and_does_not_change_plain_items() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        assert!(run.inventory.iter().all(|item| item.actor.is_none()));
        run.inventory[0].actor = Some(ActorState::configured(
            Some(3),
            Some(1),
            true,
            true,
            true,
            true,
        ));
        assert!(
            run.inventory[0]
                .actor
                .as_ref()
                .is_some_and(ActorState::is_combatant)
        );
        assert!(run.inventory[1..].iter().all(|item| item.actor.is_none()));
    }

    #[test]
    fn plain_items_do_not_resolve_through_the_runtime_actor_surface() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;
        let item_id = run.inventory[0].instance_id.clone();

        assert!(
            run.runtime_actor(&item_id).is_none(),
            "plain items must not acquire an actor-shaped contract by lookup alone"
        );
    }

    #[test]
    fn named_actor_attack_intent_resolves_even_when_the_phrase_starts_with_attack() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let location_id = run.current_location_id.clone();
        let target_id = run.location_enemies[&location_id][0].clone();
        let target_name = run
            .enemy_instances
            .get(&target_id)
            .map(|enemy| enemy.name.clone())
            .unwrap_or_else(|| target_id.clone());
        run.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "test_actor".to_owned(),
                template_id: "test_actor".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Lira".to_owned(),
                description: "A reducer-backed combat fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState::configured(Some(3), Some(5), true, true, false, true),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );

        let resolved = crate::game::combat::resolve_actor_attack_intent(
            &run,
            &format!("attack {target_name} with Lira"),
        )
        .expect("the phrase contains a bounded actor instruction")
        .expect("the combat-capable actor and hostile target resolve");

        assert_eq!(resolved, ("test_actor".to_owned(), target_id));
    }

    #[test]
    fn controlled_player_actor_move_bridges_to_the_existing_movement_reducer() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let origin = run.current_location_id.clone();
        let destination = bundle
            .locations
            .iter()
            .find(|location| location.id == origin)
            .and_then(|location| location.connections.first())
            .cloned()
            .expect("fixture start location has an exit");
        let player_id = run.player_entity_id.clone();

        let outcome = crate::game::reducer::apply_action(
            &mut run,
            &bundle,
            crate::game::GameAction::ActorMove {
                origin_id: player_id,
                destination: destination.clone(),
            },
        );

        assert_eq!(run.current_location_id, destination);
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            crate::game::GameEvent::Moved { from_location_id, to_location_id, .. }
                if from_location_id == &origin && to_location_id == &destination
        )));
    }

    #[test]
    fn actor_capable_item_is_a_generic_inventory_endpoint() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let mut container = run.inventory.remove(0);
        let carried = run.inventory.remove(0);
        let carried_id = carried.id.clone();
        let carried_name = carried.name.clone();
        let container_id = container.instance_id.clone();
        let container_name = container.name.clone();
        container.actor = Some(ActorState {
            capabilities: vec!["inventory".to_owned()],
            ..Default::default()
        });
        run.inventory.push(container);
        run.inventory.push(carried);

        run.transfer_owned_item(
            &carried_id,
            &super::ItemOwner::Player,
            &super::ItemOwner::Entity(container_id.clone()),
        )
        .expect("an inventory-capable item can receive a real transferred item");

        let container = run
            .inventory
            .iter()
            .find(|entry| entry.instance_id == container_id)
            .expect("container remains a player-held entity");
        assert!(
            container
                .inventory
                .iter()
                .any(|entry| entry.id == carried_id)
        );
        let restored: RunState =
            serde_json::from_value(serde_json::to_value(&run).expect("run serializes"))
                .expect("nested item ownership restores");
        assert!(restored.inventory.iter().any(|entry| {
            entry.instance_id == container_id
                && entry
                    .inventory
                    .iter()
                    .any(|carried| carried.id == carried_id)
        }));

        let handoff = run
            .resolve_actor_transfer_to_player(&format!("{container_name}, give me {carried_name}"))
            .expect("generic endpoint language should recognize an actor-capable item")
            .expect("the container owns the transferred item");
        assert_eq!(handoff.1, super::ItemOwner::Entity(container_id));
        run.transfer_owned_item(&handoff.0, &handoff.1, &handoff.2)
            .expect("the same generic transfer path returns an item from a container");
        assert!(run.inventory.iter().any(|entry| entry.id == carried_id));
    }

    #[test]
    fn plain_item_cannot_become_an_inventory_endpoint() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;
        let container_id = run.inventory[0].instance_id.clone();
        let carried_id = run.inventory[1].id.clone();
        let mut run = run;

        let error = run
            .transfer_owned_item(
                &carried_id,
                &super::ItemOwner::Player,
                &super::ItemOwner::Entity(container_id),
            )
            .expect_err("plain items do not receive inventory mechanics");
        assert_eq!(error, "Destination does not have the inventory capability.");
    }

    #[test]
    fn item_requests_to_plain_items_are_host_rejected_before_narration() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;
        let item_name = run.inventory[0].name.clone();
        let error = run
            .resolve_actor_transfer_to_player(&format!("ask {item_name} for an item"))
            .expect("an explicit item request should be host-owned")
            .expect_err("a plain item has no transfer endpoint");
        assert!(error.contains("no inventory capability"));
    }

    #[test]
    fn item_requests_to_hostile_encounters_are_host_rejected_before_narration() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let run = crate::game::generation::generate_new_run(&bundle).state;
        let enemy = run
            .enemy_instances
            .values()
            .find(|enemy| {
                enemy.location_id == run.current_location_id
                    && enemy.lifecycle == InstanceLifecycle::Active
            })
            .expect("fixture has a live enemy at the starting location");
        let name = if enemy.name.is_empty() {
            enemy.template_id.clone()
        } else {
            enemy.name.clone()
        };
        let error = run
            .resolve_actor_transfer_to_player(&format!("ask {name} for an item"))
            .expect("a hostile item request should be host-owned")
            .expect_err("hostile encounter cannot transfer inventory");
        assert!(error.contains("hostile"));
    }

    #[test]
    fn named_inventory_actor_resolves_a_bounded_player_handoff_and_persists_it() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let held_item = run.inventory.remove(0);
        let item_id = held_item.id.clone();
        let item_name = held_item.name.clone();
        let location_id = run.current_location_id.clone();
        run.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "test_carrier".to_owned(),
                template_id: "test_carrier".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Carrier".to_owned(),
                description: "A reliable pack mule.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: vec![held_item],
                equipped_item_id: None,
            }],
        );

        let transfer = run
            .resolve_actor_transfer_to_player(&format!("Carrier, give me {item_name}"))
            .expect("handoff language should target a named actor")
            .expect("actor owns the requested item");
        assert_eq!(transfer.0, item_id);
        assert_eq!(
            transfer.1,
            super::ItemOwner::Entity("test_carrier".to_owned())
        );
        assert_eq!(transfer.2, super::ItemOwner::Player);

        run.transfer_owned_item(&transfer.0, &transfer.1, &transfer.2)
            .expect("host validates the requested handoff");
        let reverse = run
            .resolve_player_transfer_to_actor(&format!("give {item_name} to Carrier"))
            .expect("player-to-actor language should resolve")
            .expect("the player now owns the item");
        assert_eq!(reverse.0, item_id);
        assert_eq!(reverse.1, super::ItemOwner::Player);
        assert_eq!(
            reverse.2,
            super::ItemOwner::Entity("test_carrier".to_owned())
        );
        run.transfer_owned_item(&reverse.0, &reverse.1, &reverse.2)
            .expect("same primitive returns the item to the actor");

        let restored: RunState =
            serde_json::from_value(serde_json::to_value(&run).expect("run serializes"))
                .expect("run with actor inventory restores");
        assert!(
            restored
                .location_npcs
                .values()
                .flatten()
                .find(|npc| npc.instance_id == "test_carrier")
                .is_some_and(|npc| npc.inventory.iter().any(|item| item.id == item_id))
        );
    }

    #[test]
    fn named_take_phrase_uses_the_same_real_player_to_actor_transfer() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let item_id = run.inventory[0].id.clone();
        let item_name = run.inventory[0].name.clone();
        let location_id = run.current_location_id.clone();
        run.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "generic_receiver".to_owned(),
                template_id: "generic_receiver".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Receiver".to_owned(),
                description: "A capability fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["trusting".to_owned()],
                disposition: ActorDisposition::Neutral,
                actor: ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );

        let transfer = run
            .resolve_player_transfer_to_actor(&format!("Receiver, take my {item_name}"))
            .expect("named take is bounded transfer language")
            .expect("the receiver can carry inventory");
        assert_eq!(transfer.0, item_id);
        run.transfer_owned_item(&transfer.0, &transfer.1, &transfer.2)
            .expect("transfer commits exactly once");
        assert!(!run.inventory.iter().any(|item| item.id == item_id));
        assert!(
            run.entity_inventory(&transfer.2)
                .is_some_and(|items| items.iter().any(|item| item.id == item_id))
        );

        let returned = run
            .resolve_actor_transfer_to_player(&format!("Receiver, give my {item_name} back"))
            .expect("same endpoint supports the reverse direction")
            .expect("receiver owns the item");
        run.transfer_owned_item(&returned.0, &returned.1, &returned.2)
            .expect("return uses the shared primitive");
        assert!(run.inventory.iter().any(|item| item.id == item_id));
    }

    #[test]
    fn social_state_is_reversible_tag_driven_and_saveable() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let location_id = run.current_location_id.clone();
        run.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "social_actor".to_owned(),
                template_id: "social_actor".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Social actor".to_owned(),
                description: String::new(),
                faction: "test".to_owned(),
                tags: vec!["trusting".to_owned()],
                disposition: ActorDisposition::Neutral,
                actor: ActorState::default(),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        for _ in 0..2 {
            run.apply_social_event("social_actor", super::SocialEventKind::Gift)
                .expect("social event");
        }
        assert_eq!(
            run.runtime_actor("social_actor")
                .expect("actor")
                .disposition,
            ActorDisposition::Friendly
        );
        for _ in 0..6 {
            run.apply_social_event("social_actor", super::SocialEventKind::Insult)
                .expect("social event");
        }
        assert_eq!(
            run.runtime_actor("social_actor")
                .expect("actor")
                .disposition,
            ActorDisposition::Hostile
        );
        let restored: RunState =
            serde_json::from_value(serde_json::to_value(&run).expect("serialize"))
                .expect("restore social state");
        assert_eq!(
            restored
                .social_state("social_actor")
                .expect("persisted")
                .relationship_score,
            -4
        );
    }

    #[test]
    fn bounded_social_language_and_theft_resolution_stay_host_bounded() {
        let bundle =
            crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle");
        let mut run = crate::game::generation::generate_new_run(&bundle).state;
        let location_id = run.current_location_id.clone();
        run.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "social_target".to_owned(),
                template_id: "social_target".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Target".to_owned(),
                description: String::new(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Neutral,
                actor: ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: vec![run.inventory[0].clone()],
                equipped_item_id: None,
            }],
        );
        assert_eq!(
            run.resolve_social_intent("insult Target")
                .expect("bounded social wording")
                .expect("present actor"),
            ("social_target".to_owned(), super::SocialEventKind::Insult)
        );
        assert_eq!(
            run.resolve_social_intent("fuck you, Target")
                .expect("targeted hostile language is bounded social input")
                .expect("present actor"),
            ("social_target".to_owned(), super::SocialEventKind::Insult)
        );
        assert_eq!(
            run.resolve_social_intent("attack Target with a bat")
                .expect("direct actor attack is a bounded social threat")
                .expect("present actor"),
            ("social_target".to_owned(), super::SocialEventKind::Threat)
        );
        let theft_error = run
            .resolve_social_intent("steal from Target")
            .expect("theft is recognized as a mechanical request")
            .expect_err("theft cannot become prose-only social state");
        assert!(theft_error.contains("TransferItem"));
        assert_eq!(
            run.resolve_player_theft_from_actor("steal Torch from Target")
                .expect("theft wording is recognized")
                .expect("real actor-owned item resolves"),
            (
                run.inventory[0].id.clone(),
                ItemOwner::Entity("social_target".to_owned()),
                ItemOwner::Player,
            )
        );
        let missing_item = run
            .resolve_player_theft_from_actor("steal imaginary widget from Target")
            .expect("theft wording is recognized")
            .expect_err("unowned item cannot become a transfer");
        assert!(missing_item.contains("does not own"));
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectiveState {
    #[serde(default)]
    pub instance_id: String,
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub lifecycle: InstanceLifecycle,
    #[serde(default)]
    pub bucket: ObjectiveBucket,
    pub id: String,
    pub name: String,
    pub description: String,
    pub target_boss_id: Option<String>,
    pub required_item_id: Option<String>,
    pub required_location_id: Option<String>,
    pub completed: bool,
}
