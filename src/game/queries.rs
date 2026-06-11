use crate::data::datapacks::{
    BossTemplate, DatapackBundle, EnemyTemplate, ItemTemplate, LocationTemplate,
};

use super::state::RunState;

pub fn describe_current_location(state: &RunState, bundle: &DatapackBundle) -> Vec<String> {
    let Some(location) = find_location(bundle, &state.current_location_id) else {
        return vec!["Current location could not be resolved.".to_owned()];
    };

    let mut lines = vec![format!("{}: {}", location.name, location.description)];

    if !location.connections.is_empty() {
        let exits = location
            .connections
            .iter()
            .filter_map(|id| find_location(bundle, id).map(|location| location.name.clone()))
            .collect::<Vec<_>>();
        lines.push(format!("Connections: {}.", exits.join(", ")));
    }

    if let Some(items) = state.location_items.get(&location.id)
        && !items.is_empty()
    {
        let item_names = items
            .iter()
            .filter_map(|id| find_item(bundle, id).map(|item| item.name.clone()))
            .collect::<Vec<_>>();
        if !item_names.is_empty() {
            lines.push(format!("Items here: {}.", item_names.join(", ")));
        }
    }

    if let Some(enemies) = state.location_enemies.get(&location.id)
        && !enemies.is_empty()
    {
        let enemy_names = enemies
            .iter()
            .filter(|id| state.enemies_alive.contains(*id))
            .filter_map(|id| find_enemy(bundle, id).map(|enemy| enemy.name.clone()))
            .collect::<Vec<_>>();
        if !enemy_names.is_empty() {
            lines.push(format!("Enemies here: {}.", enemy_names.join(", ")));
        }
    }

    if let Some(bosses) = state.location_bosses.get(&location.id)
        && !bosses.is_empty()
    {
        let boss_names = bosses
            .iter()
            .filter(|id| state.bosses_alive.contains(*id))
            .filter_map(|id| find_boss(bundle, id).map(|boss| boss.name.clone()))
            .collect::<Vec<_>>();
        if !boss_names.is_empty() {
            lines.push(format!("Boss here: {}.", boss_names.join(", ")));
        }
    }

    lines
}

pub fn find_location<'a>(bundle: &'a DatapackBundle, id: &str) -> Option<&'a LocationTemplate> {
    bundle.locations.iter().find(|location| location.id == id)
}

pub fn find_location_by_name_or_id<'a>(
    bundle: &'a DatapackBundle,
    query: &str,
) -> Option<&'a LocationTemplate> {
    bundle
        .locations
        .iter()
        .find(|location| matches_name(query, &location.id, &location.name))
}

pub fn find_item<'a>(bundle: &'a DatapackBundle, id: &str) -> Option<&'a ItemTemplate> {
    bundle.items.iter().find(|item| item.id == id)
}

pub fn find_item_by_name_or_id<'a>(
    bundle: &'a DatapackBundle,
    query: &str,
) -> Option<&'a ItemTemplate> {
    bundle
        .items
        .iter()
        .find(|item| matches_name(query, &item.id, &item.name))
}

pub fn find_enemy<'a>(bundle: &'a DatapackBundle, id: &str) -> Option<&'a EnemyTemplate> {
    bundle.enemies.iter().find(|enemy| enemy.id == id)
}

pub fn find_enemy_by_name_or_id<'a>(
    bundle: &'a DatapackBundle,
    query: &str,
) -> Option<&'a EnemyTemplate> {
    bundle
        .enemies
        .iter()
        .find(|enemy| matches_name(query, &enemy.id, &enemy.name))
}

pub fn find_boss<'a>(bundle: &'a DatapackBundle, id: &str) -> Option<&'a BossTemplate> {
    bundle.bosses.iter().find(|boss| boss.id == id)
}

pub fn find_boss_by_name_or_id<'a>(
    bundle: &'a DatapackBundle,
    query: &str,
) -> Option<&'a BossTemplate> {
    bundle
        .bosses
        .iter()
        .find(|boss| matches_name(query, &boss.id, &boss.name))
}

pub fn equipped_damage(state: &RunState) -> i32 {
    state
        .equipped_item_id
        .as_deref()
        .and_then(|item_id| {
            state
                .inventory
                .iter()
                .find(|item| item.id == item_id)
                .map(|item| item.damage)
        })
        .unwrap_or(1)
}

pub fn matches_name(query: &str, id: &str, name: &str) -> bool {
    let normalized_query = normalize_name(query);
    normalized_query == normalize_name(id) || normalized_query == normalize_name(name)
}

fn normalize_name(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
