use crate::data::datapacks::{
    BossTemplate, DatapackBundle, EnemyTemplate, ItemTemplate, LocationTemplate,
};

use super::state::RunState;

pub fn describe_current_location(state: &RunState, bundle: &DatapackBundle) -> Vec<String> {
    let Some(location) = find_location(bundle, &state.current_location_id) else {
        return vec!["Current location could not be resolved.".to_owned()];
    };

    let mut lines = describe_location(state, bundle, location);
    lines.extend(current_location_context_lines(state, location));
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
        let lock_state = if state.locked_locations.contains(&location.id) {
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
                    if state.locked_locations.contains(&location.id) {
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

fn current_location_context_lines(state: &RunState, location: &LocationTemplate) -> Vec<String> {
    let mut lines = Vec::new();

    if state.active_objective.required_location_id.as_deref() == Some(location.id.as_str())
        && !state.active_objective.completed
    {
        lines.push(
            "Objective pressure: this room satisfies the location requirement if you can hold it."
                .to_owned(),
        );
    }

    match location.id.as_str() {
        "front_verandah" => {
            if !state.enemies_alive.contains("shambler_front_gate") {
                lines.push(
                    "Situation: the front threshold is still a mess, but at least it belongs to you again."
                        .to_owned(),
                );
            } else if state.barricaded_locations.contains(&location.id) {
                lines.push(
                    "Situation: the threshold is still ugly, but the barricade makes it feel survivable."
                        .to_owned(),
                );
            } else if state.noise_level >= 2 {
                lines.push(
                    "Situation: every loose sound on the property seems to funnel back toward the front step."
                        .to_owned(),
                );
            }
        }
        "back_garden" => {
            if !state.enemies_alive.contains("crawler_in_weeds") {
                lines.push(
                    "Situation: the yard is still mean-looking, but the worst thing in the weeds is finally gone."
                        .to_owned(),
                );
            } else if state.barricaded_locations.contains(&location.id)
                && location.barricade_heal > 0
            {
                lines.push(format!(
                    "Situation: with the back edge secured, this becomes a rare place to recover {} HP and regroup.",
                    location.barricade_heal
                ));
            } else if state.noise_level >= 2 {
                lines.push(
                    "Situation: the yard is too open for this much noise; anything low to the ground gets closer for free."
                        .to_owned(),
                );
            }
        }
        "garage" => {
            let boss_is_here = state
                .location_bosses
                .get(&location.id)
                .into_iter()
                .flatten()
                .any(|boss_id| state.bosses_alive.contains(boss_id));
            let brute_wounded = state
                .boss_hp
                .get("brute_in_garage")
                .is_some_and(|remaining_hp| *remaining_hp > 0 && *remaining_hp <= 4);

            if brute_wounded && property_siege_lanes_secured(state) {
                lines.push(
                    "Situation: the brute is wounded, but both exposed approaches are barricaded. The garage is still ugly, just no longer backed by the whole property."
                        .to_owned(),
                );
            } else if brute_wounded {
                lines.push(
                    "Situation: the brute is hurt enough to get sloppy and strong at the same time. The garage feels smaller by the second."
                        .to_owned(),
                );
            } else if boss_is_here && property_siege_lanes_secured(state) {
                lines.push(
                    "Situation: both exposed approaches are barricaded before the finale. The brute still owns the room, but the siege pressure has lost a little bite."
                        .to_owned(),
                );
            } else if boss_is_here && state.barricaded_locations.contains("front_verandah") {
                lines.push(
                    "Situation: the front barricade is buying you time while you deal with the real problem in here."
                        .to_owned(),
                );
            } else if boss_is_here && state.noise_level >= 2 {
                lines.push(
                    "Situation: the noise outside makes the garage feel less like shelter and more like a deadline."
                        .to_owned(),
                );
            }
        }
        _ => {}
    }

    lines
}

fn normalize_name(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .replace(['_', '-', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn property_siege_lanes_secured(state: &RunState) -> bool {
    state.barricaded_locations.contains("front_verandah")
        && state.barricaded_locations.contains("back_garden")
}
