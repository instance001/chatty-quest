use crate::data::datapacks::{
    BossTemplate, DatapackBundle, EnemyTemplate, ItemTemplate, LocationTemplate,
};

use super::derived::{boss_wounded_phase_active, finale_security_secured};
use super::state::RunState;

pub fn describe_current_location(state: &RunState, bundle: &DatapackBundle) -> Vec<String> {
    let Some(location) = find_location(bundle, &state.current_location_id) else {
        return vec!["Current location could not be resolved.".to_owned()];
    };

    let mut lines = describe_location(state, bundle, location);
    lines.extend(current_location_context_lines(state, bundle, location));
    lines
}

pub fn describe_location(
    state: &RunState,
    bundle: &DatapackBundle,
    location: &LocationTemplate,
) -> Vec<String> {
    let mut lines = vec![format!(
        "{}: {}",
        location.name,
        location_description_for_state(state, location)
    )];

    if location.locked {
        let lock_state = if state.broken_locked_locations.contains(&location.id) {
            "Broken"
        } else if state.locked_locations.contains(&location.id) {
            "Locked"
        } else {
            "Unlocked"
        };
        let mut lock_line = format!("Gate state: {}", lock_state);
        if let Some(unlock_item_id) = location.unlock_item_id.as_deref() {
            lock_line.push_str(&format!(" | unlock item: {}", unlock_item_id));
        }
        lines.push(format!("{}.", lock_line));
    }

    if location.barricadable {
        let barricade_state = if state.barricaded_locations.contains(&location.id) {
            "Barricaded"
        } else {
            "Unbarricaded"
        };
        let mut barricade_line = format!("Barricade state: {}", barricade_state);
        if let Some(barricade_item_id) = location.barricade_item_id.as_deref() {
            barricade_line.push_str(&format!(" | required item: {}", barricade_item_id));
        }
        if location.barricade_heal > 0 {
            barricade_line.push_str(&format!(" | recovery bonus: {}", location.barricade_heal));
        }
        if location.barricade_blocks_retaliation {
            barricade_line.push_str(" | cover bonus: blocks direct retaliation");
        }
        if location.barricade_attack_bonus > 0 {
            barricade_line.push_str(&format!(
                " | attack bonus: {}",
                location.barricade_attack_bonus
            ));
        }
        lines.push(format!("{}.", barricade_line));
    }

    if state.active_objective.completed
        && let Some(epilogue_hook) = location.epilogue_hook.as_deref()
    {
        lines.push(format!("Aftermath hook: {}.", epilogue_hook));
    }

    if !location.connections.is_empty() {
        let exits = location
            .connections
            .iter()
            .filter_map(|id| {
                find_location(bundle, id).map(|location| {
                    if state.broken_locked_locations.contains(&location.id) {
                        format!("{} [broken]", location.name)
                    } else if state.locked_locations.contains(&location.id) {
                        format!("{} [locked]", location.name)
                    } else {
                        location.name.clone()
                    }
                })
            })
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

pub fn unlock_targets_for_item(bundle: &DatapackBundle, item_id: &str) -> Vec<String> {
    bundle
        .locations
        .iter()
        .filter(|location| location.unlock_item_id.as_deref() == Some(item_id))
        .map(|location| location.name.clone())
        .collect()
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
    let template_id = enemy_template_id(id);
    bundle.enemies.iter().find(|enemy| enemy.id == template_id)
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

pub fn enemy_template_id(enemy_id: &str) -> &str {
    enemy_id
        .strip_prefix("noise_spawn_")
        .and_then(|spawned| spawned.split_once('_').map(|(_, template_id)| template_id))
        .unwrap_or(enemy_id)
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

pub fn is_location_locked(state: &RunState, location_id: &str) -> bool {
    state.locked_locations.contains(location_id)
}

pub fn is_location_barricaded(state: &RunState, location_id: &str) -> bool {
    state.barricaded_locations.contains(location_id)
}

pub fn location_description_for_state<'a>(
    state: &RunState,
    location: &'a LocationTemplate,
) -> &'a str {
    if state.active_objective.completed {
        location
            .epilogue_description
            .as_deref()
            .unwrap_or(location.description.as_str())
    } else {
        location.description.as_str()
    }
}

pub fn matches_name(query: &str, id: &str, name: &str) -> bool {
    let normalized_query = normalize_name(query);
    normalized_query == normalize_name(id) || normalized_query == normalize_name(name)
}

fn current_location_context_lines(
    state: &RunState,
    bundle: &DatapackBundle,
    location: &LocationTemplate,
) -> Vec<String> {
    let mut lines = Vec::new();

    if state.active_objective.required_location_id.as_deref() == Some(location.id.as_str())
        && !state.active_objective.completed
    {
        lines.push(
            "Objective pressure: this room satisfies the location requirement if you can hold it."
                .to_owned(),
        );
    }

    if let Some(line) = location_situation_line(state, bundle, location) {
        lines.push(line);
    }

    lines
}

fn location_situation_line(
    state: &RunState,
    bundle: &DatapackBundle,
    location: &LocationTemplate,
) -> Option<String> {
    if let Some(line) = pressure_situation_line(state, location) {
        return Some(line);
    }

    boss_situation_line(state, bundle, location)
}

fn pressure_situation_line(state: &RunState, location: &LocationTemplate) -> Option<String> {
    let pressure_enemy_id = location.passive_pressure_enemy_id.as_deref()?;
    let line = if !state.enemies_alive.contains(pressure_enemy_id) {
        location.situation_enemy_cleared_line.as_deref()
    } else if state.barricaded_locations.contains(&location.id) {
        location.situation_barricaded_line.as_deref()
    } else if state.noise_level >= 2 {
        location.situation_high_noise_line.as_deref()
    } else {
        None
    }?;
    Some(render_location_situation_line(line, location))
}

fn boss_situation_line(
    state: &RunState,
    bundle: &DatapackBundle,
    location: &LocationTemplate,
) -> Option<String> {
    let live_boss_ids = state
        .location_bosses
        .get(&location.id)
        .into_iter()
        .flatten()
        .filter(|boss_id| state.bosses_alive.contains(*boss_id))
        .collect::<Vec<_>>();
    if live_boss_ids.is_empty() {
        return None;
    }

    let boss_wounded = live_boss_ids.iter().any(|boss_id| {
        let Some(boss) = find_boss(bundle, boss_id) else {
            return false;
        };
        let remaining_hp = state.boss_hp.get(*boss_id).copied().unwrap_or(0);
        boss_wounded_phase_active(boss, remaining_hp)
    });

    let line = if boss_wounded && finale_security_secured(state, bundle) {
        location.situation_boss_wounded_secured_line.as_deref()
    } else if boss_wounded {
        location.situation_boss_wounded_line.as_deref()
    } else if finale_security_secured(state, bundle) {
        location.situation_boss_secured_line.as_deref()
    } else if finale_security_partially_secured(state, bundle) {
        location.situation_boss_partially_secured_line.as_deref()
    } else if state.noise_level >= 2 {
        location.situation_high_noise_line.as_deref()
    } else {
        None
    }?;

    Some(render_location_situation_line(line, location))
}

fn finale_security_partially_secured(state: &RunState, bundle: &DatapackBundle) -> bool {
    bundle
        .rules
        .finale_secured_location_ids
        .iter()
        .any(|location_id| state.barricaded_locations.contains(location_id))
}

fn render_location_situation_line(line: &str, location: &LocationTemplate) -> String {
    line.replace("{barricade_heal}", &location.barricade_heal.to_string())
}

fn normalize_name(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
