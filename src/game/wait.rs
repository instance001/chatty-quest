use crate::data::datapacks::{DatapackBundle, LocationTemplate};

use super::actions::{ActionOutcome, GameEvent};
use super::noise::exposed_noise_pressure_damage;
use super::queries::{find_location, is_location_barricaded};
use super::state::RunState;

pub(super) fn handle_wait(state: &mut RunState, bundle: &DatapackBundle) -> ActionOutcome {
    let location = find_location(bundle, &state.current_location_id);
    let location_name = location
        .as_ref()
        .map(|location| location.name.clone())
        .unwrap_or_else(|| state.current_location_id.clone());

    let mut events = vec![GameEvent::Waited {
        origin_id: state.player_entity_id.clone(),
        location_id: state.current_location_id.clone(),
    }];
    let mut lines = vec![format!(
        "You wait at {} and listen to the property complain around you.",
        location_name
    )];

    if state.controlled_actor_is_alive()
        && let Some(location) = location.as_ref()
    {
        apply_location_passive_pressure(state, location, &mut events, &mut lines);
    } else if !state.controlled_actor_is_alive() {
        lines.push(
            "You remain incapacitated while the rest of the world takes its turn.".to_owned(),
        );
    }

    ActionOutcome { events, lines }
}

fn apply_location_passive_pressure(
    state: &mut RunState,
    location: &LocationTemplate,
    events: &mut Vec<GameEvent>,
    lines: &mut Vec<String>,
) {
    let Some(enemy_id) = location.passive_pressure_enemy_id.as_deref() else {
        return;
    };
    if !state.enemies_alive.contains(enemy_id) {
        return;
    }

    if is_location_barricaded(state, &location.id) {
        if let Some(line) = location.passive_pressure_blocked_line.as_deref() {
            lines.push(line.to_owned());
        }
        return;
    }

    let pressure = exposed_noise_pressure_damage(state);
    let target_id = state.player_entity_id.clone();
    let Ok((_, applied, remaining_hp, maximum_hp)) =
        state.damage_runtime_actor_health(&target_id, pressure)
    else {
        lines.push("The pressure cannot harm the controlled actor's current contract.".to_owned());
        return;
    };
    events.push(GameEvent::DamageTaken {
        origin_id: enemy_id.to_owned(),
        target_id,
        amount: applied,
        remaining_hp,
    });
    if let Some(line) = location.passive_pressure_damage_line.as_deref() {
        lines.push(line.to_owned());
    }
    if pressure > 1
        && let Some(line) = location.passive_pressure_high_noise_line.as_deref()
    {
        lines.push(line.to_owned());
    }
    lines.push(format!("HP is now {} / {}.", remaining_hp, maximum_hp));
}
