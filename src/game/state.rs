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
    pub target_boss_id: String,
    pub completed: bool,
}
