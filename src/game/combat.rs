use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, EncounterKind, GameEvent};
use super::derived::{
    boss_wounded_phase_active, boss_wounded_phase_damage_bonus,
    finale_security_retaliation_reduction,
};
use super::queries::{equipped_damage, find_boss, find_enemy, find_location};
use super::state::RunState;

pub(super) fn handle_attack(state: &mut RunState, bundle: &DatapackBundle) -> ActionOutcome {
    let current_location = state.current_location_id.clone();

    let enemy_here = state
        .location_enemies
        .get(&current_location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| state.enemies_alive.contains(*id))
                .cloned()
        });
    let boss_here = state
        .location_bosses
        .get(&current_location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| state.bosses_alive.contains(*id))
                .cloned()
        });

    if let Some(enemy_id) = enemy_here {
        let barricade_attack_bonus =
            find_location(bundle, &current_location).map_or(0, |location| {
                if state.barricaded_locations.contains(&location.id) {
                    location.barricade_attack_bonus
                } else {
                    0
                }
            });
        let player_damage = (equipped_damage(state) + barricade_attack_bonus).max(1);
        let enemy_damage = state.enemy_hp.entry(enemy_id.clone()).or_insert(0);
        *enemy_damage -= player_damage;

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        if barricade_attack_bonus > 0 {
            lines.push(format!(
                "The barricade gives you a steadier angle on the threat. Attack bonus: +{}.",
                barricade_attack_bonus
            ));
        }
        let mut events = vec![GameEvent::AttackResolved {
            target_id: enemy_id.clone(),
            target_kind: EncounterKind::Enemy,
            damage: player_damage,
            defeated: *enemy_damage <= 0,
        }];

        if *enemy_damage <= 0 {
            state.enemies_alive.remove(&enemy_id);
            state.enemies_defeated.insert(enemy_id.clone());
            state.spawned_enemy_targets.remove(&enemy_id);
            state.spawned_enemy_origins.remove(&enemy_id);
            state.spawned_enemy_searching.remove(&enemy_id);
            state.spawned_enemy_sight_targets.remove(&enemy_id);
            state.spawned_enemy_sight_subjects.remove(&enemy_id);
            state.spawned_enemy_sight_delays.remove(&enemy_id);
            if let Some(entries) = state.location_enemies.get_mut(&current_location) {
                entries.retain(|entry| entry != &enemy_id);
            }
            let enemy_name = find_enemy(bundle, &enemy_id)
                .map(|enemy| enemy.name.clone())
                .unwrap_or_else(|| enemy_id.clone());
            lines.push(format!("{} goes down.", enemy_name));
            if let Some(defeat_line) =
                find_enemy(bundle, &enemy_id).and_then(|enemy| enemy.defeat_line.clone())
            {
                lines.push(defeat_line);
            }
        } else {
            let retaliation_blocked =
                find_location(bundle, &current_location).is_some_and(|location| {
                    location.barricade_blocks_retaliation
                        && state.barricaded_locations.contains(&location.id)
                });

            if retaliation_blocked {
                lines.push(
                    "The barricade keeps the threat at splinter-spitting distance. It cannot land the hit cleanly."
                        .to_owned(),
                );
            } else {
                let retaliation_bonus =
                    exposed_noise_retaliation_bonus(state, bundle, &current_location);
                let retaliation = find_enemy(bundle, &enemy_id)
                    .map(|enemy| enemy.damage)
                    .unwrap_or(1)
                    + retaliation_bonus;
                state.hp = (state.hp - retaliation).max(0);
                events.push(GameEvent::DamageTaken {
                    amount: retaliation,
                    remaining_hp: state.hp,
                });
                lines.push(format!("The enemy hits back for {} damage.", retaliation));
                if let Some(retaliation_line) =
                    find_enemy(bundle, &enemy_id).and_then(|enemy| enemy.retaliation_line.clone())
                {
                    lines.push(retaliation_line);
                }
                lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
            }
        }

        return ActionOutcome { events, lines };
    }

    if let Some(boss_id) = boss_here {
        let player_damage = equipped_damage(state).max(1);
        let boss_template = find_boss(bundle, &boss_id);
        let boss_damage = state.boss_hp.entry(boss_id.clone()).or_insert(0);
        *boss_damage -= player_damage;
        let boss_remaining_hp = *boss_damage;
        let wounded_phase =
            boss_template.is_some_and(|boss| boss_wounded_phase_active(boss, boss_remaining_hp));

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        let mut events = vec![GameEvent::AttackResolved {
            target_id: boss_id.clone(),
            target_kind: EncounterKind::Boss,
            damage: player_damage,
            defeated: boss_remaining_hp <= 0,
        }];

        if boss_remaining_hp <= 0 {
            state.bosses_alive.remove(&boss_id);
            state.bosses_defeated.insert(boss_id.clone());
            if let Some(entries) = state.location_bosses.get_mut(&current_location) {
                entries.retain(|entry| entry != &boss_id);
            }
            let boss_name = find_boss(bundle, &boss_id)
                .map(|boss| boss.name.clone())
                .unwrap_or_else(|| boss_id.clone());
            lines.push(
                boss_template
                    .and_then(|boss| boss.defeat_line.as_deref())
                    .map(|line| render_boss_combat_line(line, &boss_name, player_damage, 0))
                    .unwrap_or_else(|| {
                        format!(
                            "{} collapses. The worst thing on the block is finished.",
                            boss_name
                        )
                    }),
            );
            if state.active_objective.required_location_id.as_deref()
                == Some(state.current_location_id.as_str())
                && let Some(line) = find_location(bundle, &state.current_location_id)
                    .and_then(|location| location.boss_defeated_objective_line.as_deref())
            {
                lines.push(line.to_owned());
            }
        } else {
            if wounded_phase {
                lines.push(
                    boss_template
                        .and_then(|boss| boss.wounded_phase_combat_line.clone())
                        .unwrap_or_else(|| {
                            "The boss enters a wounded final phase and becomes more dangerous."
                                .to_owned()
                        }),
                );
            }
            let secured_property_bonus =
                finale_security_retaliation_reduction(state, bundle, &boss_id);
            let wounded_bonus = boss_template
                .map(boss_wounded_phase_damage_bonus)
                .filter(|_| wounded_phase)
                .unwrap_or(0);
            let retaliation = (boss_template.map(|boss| boss.damage).unwrap_or(2) + wounded_bonus
                - secured_property_bonus)
                .max(1);
            state.hp = (state.hp - retaliation).max(0);
            events.push(GameEvent::DamageTaken {
                amount: retaliation,
                remaining_hp: state.hp,
            });
            let boss_name = boss_template
                .map(|boss| boss.name.as_str())
                .unwrap_or("The boss");
            lines.push(
                boss_template
                    .and_then(|boss| boss.retaliation_line.as_deref())
                    .map(|line| render_boss_combat_line(line, boss_name, retaliation, 0))
                    .unwrap_or_else(|| {
                        format!("The boss smashes back for {} damage.", retaliation)
                    }),
            );
            if secured_property_bonus > 0
                && let Some(line) =
                    boss_template.and_then(|boss| boss.finale_security_retaliation_line.as_deref())
            {
                lines.push(render_boss_combat_line(
                    line,
                    boss_name,
                    retaliation,
                    secured_property_bonus,
                ));
            }
            if let Some(line) = find_location(bundle, &state.current_location_id)
                .and_then(|location| location.boss_retaliation_context_line.as_deref())
            {
                lines.push(line.to_owned());
            }
            if wounded_phase {
                lines.push(
                    boss_template
                        .and_then(|boss| boss.wounded_phase_retaliation_line.clone())
                        .unwrap_or_else(|| {
                            "Final-phase pressure: the boss is hitting harder now.".to_owned()
                        }),
                );
            }
            lines.push(format!("HP is now {} / {}.", state.hp, state.max_hp));
        }

        return ActionOutcome { events, lines };
    }

    ActionOutcome {
        events: vec![GameEvent::AttackWhiff],
        lines: vec!["You swing at the air with admirable commitment.".to_owned()],
    }
}

fn render_boss_combat_line(line: &str, boss_name: &str, damage: i32, reduction: i32) -> String {
    line.replace("{boss_name}", boss_name)
        .replace("{damage}", &damage.to_string())
        .replace("{reduction}", &reduction.to_string())
}

fn exposed_noise_retaliation_bonus(
    state: &RunState,
    bundle: &DatapackBundle,
    location_id: &str,
) -> i32 {
    let Some(location) = find_location(bundle, location_id) else {
        return 0;
    };
    if state.noise_level >= 2
        && !state.barricaded_locations.contains(location_id)
        && location.tags.iter().any(|tag| tag == "noise_pressure")
    {
        1
    } else {
        0
    }
}
