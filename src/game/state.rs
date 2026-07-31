use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunState {
    pub datapack_id: String,
    pub datapack_display_name: String,
    pub current_location_id: String,
    pub known_locations: HashSet<String>,
    pub visited_locations: HashSet<String>,
    pub inventory: Vec<InventoryEntry>,
    pub equipped_item_id: Option<String>,
    pub hp: i32,
    pub max_hp: i32,
    pub active_objective: ObjectiveState,
    pub enemies_alive: HashSet<String>,
    pub enemies_defeated: HashSet<String>,
    pub enemy_hp: HashMap<String, i32>,
    pub bosses_alive: HashSet<String>,
    pub bosses_defeated: HashSet<String>,
    pub boss_hp: HashMap<String, i32>,
    pub location_items: HashMap<String, Vec<String>>,
    pub location_enemies: HashMap<String, Vec<String>>,
    pub location_bosses: HashMap<String, Vec<String>>,
    pub locked_locations: HashSet<String>,
    #[serde(default)]
    pub broken_locked_locations: HashSet<String>,
    pub barricaded_locations: HashSet<String>,
    #[serde(default)]
    pub turn_index: u64,
    pub noise_level: i32,
    #[serde(default)]
    pub noise_spawn_count: u32,
    #[serde(default)]
    pub spawned_enemy_targets: HashMap<String, String>,
    #[serde(default)]
    pub spawned_enemy_origins: HashMap<String, String>,
    #[serde(default)]
    pub spawned_enemy_searching: HashSet<String>,
    #[serde(default)]
    pub spawned_enemy_sight_targets: HashMap<String, String>,
    #[serde(default)]
    pub spawned_enemy_sight_subjects: HashMap<String, String>,
    #[serde(default)]
    pub spawned_enemy_sight_delays: HashMap<String, u8>,
    pub boundary_response: Option<String>,
    pub rolling_summary: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub damage: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectiveState {
    pub id: String,
    pub name: String,
    pub description: String,
    pub target_boss_id: Option<String>,
    pub required_item_id: Option<String>,
    pub required_location_id: Option<String>,
    pub completed: bool,
}
