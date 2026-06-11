use std::collections::{HashMap, HashSet};

use crate::data::datapacks::{DatapackBundle, ItemTemplate, ObjectiveTemplate};

use super::state::{InventoryEntry, ObjectiveState, RunState};

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
        id: active_objective_template.id.clone(),
        name: active_objective_template.name.clone(),
        description: active_objective_template.description.clone(),
        target_boss_id: active_objective_template.target_boss_id.clone(),
        completed: false,
    };

    let current_location_id = bundle.rules.starting_location.clone();
    let known_locations = HashSet::from([current_location_id.clone()]);
    let visited_locations = HashSet::from([current_location_id.clone()]);
    let enemies_alive = bundle
        .enemies
        .iter()
        .map(|enemy| enemy.id.clone())
        .collect::<HashSet<_>>();
    let enemies_defeated = HashSet::new();
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
        "Try commands like: look, go kitchen, inspect kitchen, take medkit, equip cricket bat, use medkit, attack, wait.".to_owned(),
    ];

    if let Some(dm_style) = &bundle.dm_style {
        log_lines.push(format!("DM capsule: {}.", dm_style));
    }

    let state = RunState {
        datapack_id: bundle.pack.id.clone(),
        datapack_display_name: bundle.pack.display_name.clone(),
        current_location_id,
        known_locations,
        visited_locations,
        inventory: starter_items,
        equipped_item_id,
        hp: 10,
        max_hp: 10,
        active_objective,
        enemies_alive,
        enemies_defeated,
        enemy_hp,
        bosses_alive,
        bosses_defeated,
        boss_hp,
        location_items,
        location_enemies,
        location_bosses,
        boundary_response: bundle.rules.boundary_response.clone(),
        rolling_summary: vec![format!(
            "Run started for scenario '{}'.",
            bundle.pack.display_name
        )],
    };

    GeneratedRun { state, log_lines }
}

fn to_inventory_entry(item: &ItemTemplate) -> InventoryEntry {
    InventoryEntry {
        id: item.id.clone(),
        name: item.name.clone(),
        description: item.description.clone(),
        damage: item.damage,
    }
}

fn select_objective(objectives: &[ObjectiveTemplate]) -> &ObjectiveTemplate {
    objectives
        .first()
        .expect("validated datapacks must include at least one objective")
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;

    use super::generate_new_run;

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
        assert_eq!(state.active_objective.target_boss_id, "brute_in_garage");
        assert!(!state.active_objective.completed);
        assert_eq!(state.hp, 10);
        assert_eq!(state.max_hp, 10);
        assert_eq!(state.inventory.len(), 2);
        assert!(state.inventory.iter().any(|item| item.id == "torch"));
        assert!(state.inventory.iter().any(|item| item.id == "cricket_bat"));
        assert_eq!(state.equipped_item_id.as_deref(), Some("torch"));
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
    }
}
