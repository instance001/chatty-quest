use crate::data::datapacks::DatapackBundle;

use super::actions::{ActionOutcome, EncounterKind, GameEvent};
use super::derived::{
    boss_wounded_phase_active, boss_wounded_phase_damage_bonus,
    finale_security_retaliation_reduction,
};
use super::effects::{
    EffectEvaluation, adjusted_timing_cost, consume_timing_effects, evaluate_status_effects,
    evaluate_timing_effects,
};
use super::queries::{find_boss, find_enemy, find_location};
use super::state::{
    ActionTimingKind, ActorDisposition, EffectTrigger, InstanceLifecycle, RunState,
};

/// Compatibility scenario rules decorate a controlled attack, while target resolution remains
/// entity-agnostic.  A blank target preserves the classic `attack` priority; a supplied target
/// lets the same controlled actor address any hostile runtime entity by name or instance id.
fn handle_player_attack(
    state: &mut RunState,
    bundle: &DatapackBundle,
    target_query: Option<&str>,
) -> ActionOutcome {
    let current_location = state.current_location_id.clone();
    let matches_target = |id: &str, name: &str| {
        target_query.is_none_or(|query| {
            let query = query.to_ascii_lowercase();
            query.contains(&id.to_ascii_lowercase()) || query.contains(&name.to_ascii_lowercase())
        })
    };

    let enemy_here = state
        .location_enemies
        .get(&current_location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| {
                    state.enemies_alive.contains(*id)
                        && state
                            .runtime_actor(id)
                            .is_some_and(|actor| actor.actor.has_health_contract())
                        && {
                            let name = state
                                .runtime_actor(id)
                                .map(|actor| actor.name)
                                .unwrap_or_else(|| (*id).clone());
                            matches_target(id, &name)
                        }
                })
                .cloned()
        });
    let boss_here = state
        .location_bosses
        .get(&current_location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| {
                    state.bosses_alive.contains(*id)
                        && state
                            .runtime_actor(id)
                            .is_some_and(|actor| actor.actor.has_health_contract())
                        && {
                            let name = state
                                .runtime_actor(id)
                                .map(|actor| actor.name)
                                .unwrap_or_else(|| (*id).clone());
                            matches_target(id, &name)
                        }
                })
                .cloned()
        });

    if let Some(enemy_id) = enemy_here {
        let target_tags = state
            .runtime_actor(&enemy_id)
            .map(|actor| actor.tags)
            .or_else(|| find_enemy(bundle, &enemy_id).map(|enemy| enemy.tags.clone()))
            .unwrap_or_default();
        let context_tags = find_location(bundle, &current_location)
            .map(|location| location.tags.clone())
            .unwrap_or_default();
        let origin_id = state.player_entity_id.clone();
        let effects = evaluate_status_effects(
            state,
            &origin_id,
            EffectTrigger::Combat,
            &target_tags,
            &context_tags,
        );
        let barricade_attack_bonus =
            find_location(bundle, &current_location).map_or(0, |location| {
                if state.barricaded_locations.contains(&location.id) {
                    location.barricade_attack_bonus
                } else {
                    0
                }
            });
        let player_damage = (controlled_actor_combat_damage(state)
            + barricade_attack_bonus
            + effects.additive_damage)
            .max(1);
        let (_, enemy_name, defeated) = damage_hostile_target(state, &enemy_id, player_damage)
            .expect("selected hostile passed shared health validation");

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        if barricade_attack_bonus > 0 {
            lines.push(format!(
                "The barricade gives you a steadier angle on the threat. Attack bonus: +{}.",
                barricade_attack_bonus
            ));
        }
        if effects.additive_damage != 0 {
            lines.push(format!(
                "Active effects modify this attack by {:+} damage.",
                effects.additive_damage
            ));
        }
        let mut events = vec![GameEvent::AttackResolved {
            origin_id: "player".to_owned(),
            target_id: enemy_id.clone(),
            target_kind: EncounterKind::Enemy,
            damage: player_damage,
            defeated,
        }];
        events.extend(
            effects
                .expired_instance_ids
                .iter()
                .cloned()
                .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
        );

        if defeated {
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
            } else if state
                .runtime_actor(&enemy_id)
                .is_some_and(|actor| actor.actor.is_combatant())
            {
                let retaliation_bonus =
                    exposed_noise_retaliation_bonus(state, bundle, &current_location);
                let retaliation = state
                    .runtime_actor(&enemy_id)
                    .and_then(|actor| actor.actor.damage)
                    .or_else(|| find_enemy(bundle, &enemy_id).map(|enemy| enemy.damage))
                    .unwrap_or(1)
                    + retaliation_bonus;
                let controlled_id = state.player_entity_id.clone();
                let (_, applied, remaining_hp, maximum_hp) = state
                    .damage_runtime_actor_health(&controlled_id, retaliation)
                    .expect("combat target passed shared health validation");
                events.push(GameEvent::DamageTaken {
                    origin_id: enemy_id.clone(),
                    target_id: controlled_id,
                    amount: applied,
                    remaining_hp,
                });
                lines.push(format!("The enemy hits back for {} damage.", retaliation));
                if let Some(retaliation_line) =
                    find_enemy(bundle, &enemy_id).and_then(|enemy| enemy.retaliation_line.clone())
                {
                    lines.push(retaliation_line);
                }
                lines.push(format!("HP is now {} / {}.", remaining_hp, maximum_hp));
            } else {
                lines.push("The hostile has no combat contract and cannot retaliate.".to_owned());
            }
        }

        return ActionOutcome { events, lines };
    }

    if let Some(boss_id) = boss_here {
        let boss_template = find_boss(bundle, &boss_id);
        let target_tags = state
            .runtime_actor(&boss_id)
            .map(|actor| actor.tags)
            .or_else(|| boss_template.map(|boss| boss.tags.clone()))
            .unwrap_or_default();
        let context_tags = find_location(bundle, &current_location)
            .map(|location| location.tags.clone())
            .unwrap_or_default();
        let origin_id = state.player_entity_id.clone();
        let effects = evaluate_status_effects(
            state,
            &origin_id,
            EffectTrigger::Combat,
            &target_tags,
            &context_tags,
        );
        let player_damage =
            (controlled_actor_combat_damage(state) + effects.additive_damage).max(1);
        let (_, boss_name, defeated) = damage_hostile_target(state, &boss_id, player_damage)
            .expect("selected hostile passed shared health validation");
        let boss_remaining_hp = state.boss_hp.get(&boss_id).copied().unwrap_or_default();
        let wounded_phase =
            boss_template.is_some_and(|boss| boss_wounded_phase_active(boss, boss_remaining_hp));

        let mut lines = vec![format!("You attack for {} damage.", player_damage)];
        let mut events = vec![GameEvent::AttackResolved {
            origin_id: "player".to_owned(),
            target_id: boss_id.clone(),
            target_kind: EncounterKind::Boss,
            damage: player_damage,
            defeated,
        }];
        events.extend(
            effects
                .expired_instance_ids
                .iter()
                .cloned()
                .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
        );

        if defeated {
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
        } else if state
            .runtime_actor(&boss_id)
            .is_some_and(|actor| actor.actor.is_combatant())
        {
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
            let retaliation = (state
                .runtime_actor(&boss_id)
                .and_then(|actor| actor.actor.damage)
                .or_else(|| boss_template.map(|boss| boss.damage))
                .unwrap_or(2)
                + wounded_bonus
                - secured_property_bonus)
                .max(1);
            let controlled_id = state.player_entity_id.clone();
            let (_, applied, remaining_hp, maximum_hp) = state
                .damage_runtime_actor_health(&controlled_id, retaliation)
                .expect("combat target passed shared health validation");
            events.push(GameEvent::DamageTaken {
                origin_id: boss_id.clone(),
                target_id: controlled_id,
                amount: applied,
                remaining_hp,
            });
            let boss_name = state
                .runtime_actor(&boss_id)
                .map(|actor| actor.name)
                .or_else(|| boss_template.map(|boss| boss.name.clone()))
                .unwrap_or_else(|| "The boss".to_owned());
            lines.push(
                boss_template
                    .and_then(|boss| boss.retaliation_line.as_deref())
                    .map(|line| render_boss_combat_line(line, &boss_name, retaliation, 0))
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
                    &boss_name,
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
            lines.push(format!("HP is now {} / {}.", remaining_hp, maximum_hp));
        } else {
            lines.push("The hostile has no combat contract and cannot retaliate.".to_owned());
        }

        return ActionOutcome { events, lines };
    }

    if let Some(npc_index) = state.location_npcs.get(&current_location).and_then(|npcs| {
        npcs.iter().position(|npc| {
            npc.lifecycle == InstanceLifecycle::Active
                && npc.disposition == ActorDisposition::Hostile
                && npc.actor.is_combatant()
                && matches_target(&npc.instance_id, &npc.name)
        })
    }) {
        let target_id = state.location_npcs[&current_location][npc_index]
            .instance_id
            .clone();
        return handle_player_hostile_actor_attack(state, bundle, &current_location, &target_id);
    }

    // Actor-capable items are a normal runtime-actor bucket. They deliberately arrive after the
    // legacy encounter ordering above, so an ordinary player Attack keeps its authored scenario
    // target priority while still reaching every hostile entity class through the same reducer.
    if let Some(target_id) = first_matching_hostile(state, target_query, &current_location)
        .filter(|id| state.world_item_instances.contains_key(id))
    {
        return handle_player_hostile_actor_attack(state, bundle, &current_location, &target_id);
    }

    ActionOutcome {
        events: vec![GameEvent::AttackWhiff {
            origin_id: state.player_entity_id.clone(),
        }],
        lines: vec!["You swing at the air with admirable commitment.".to_owned()],
    }
}

/// Compatibility scenario rules (barricades, boss phases, narration) may decorate a controlled
/// attack, but its base damage is always read from the same runtime actor contract used by every
/// other origin. The fallback preserves malformed pre-actor saves without creating a second
/// authority for normal runs.
fn controlled_actor_combat_damage(state: &RunState) -> i32 {
    state
        .runtime_actor(&state.player_entity_id)
        .and_then(|actor| actor.actor.damage)
        .unwrap_or(1)
}

fn handle_player_hostile_actor_attack(
    state: &mut RunState,
    bundle: &DatapackBundle,
    current_location: &str,
    target_id: &str,
) -> ActionOutcome {
    let target = state
        .runtime_actor(target_id)
        .expect("selected hostile runtime actor must remain present");
    let target_tags = target.tags;
    let context_tags = find_location(bundle, current_location)
        .map(|location| location.tags.clone())
        .unwrap_or_default();
    let origin_id = state.player_entity_id.clone();
    let effects = evaluate_status_effects(
        state,
        &origin_id,
        EffectTrigger::Combat,
        &target_tags,
        &context_tags,
    );
    let player_damage = (controlled_actor_combat_damage(state) + effects.additive_damage).max(1);
    let retaliation = state
        .runtime_actor(target_id)
        .and_then(|actor| actor.actor.damage)
        .expect("combat-capable NPC must have damage");
    let (target_kind, target_name, defeated) =
        damage_hostile_target(state, target_id, player_damage)
            .expect("selected hostile passed shared health validation");
    let mut events = vec![GameEvent::AttackResolved {
        origin_id: "player".to_owned(),
        target_id: target_id.to_owned(),
        target_kind,
        damage: player_damage,
        defeated,
    }];
    events.extend(
        effects
            .expired_instance_ids
            .into_iter()
            .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
    );
    let mut lines = vec![format!(
        "You attack {} for {} damage.",
        target_name, player_damage
    )];
    if defeated {
        lines.push(format!("{} is defeated.", target_name));
    } else {
        let controlled_id = state.player_entity_id.clone();
        let (_, applied, remaining_hp, maximum_hp) = state
            .damage_runtime_actor_health(&controlled_id, retaliation)
            .expect("combat target passed shared health validation");
        events.push(GameEvent::DamageTaken {
            origin_id: target_id.to_owned(),
            target_id: controlled_id,
            amount: applied,
            remaining_hp,
        });
        lines.push(format!(
            "{} hits back for {} damage.",
            target_name, retaliation
        ));
        lines.push(format!("HP is now {} / {}.", remaining_hp, maximum_hp));
    }
    ActionOutcome { events, lines }
}

/// Resolves the deliberately small, host-owned natural-language seam for actor instructions.
/// This only identifies existing runtime actors and existing hostile targets; it never lets
/// narration manufacture either side of a combat action.
pub fn resolve_actor_attack_intent(
    state: &RunState,
    input: &str,
) -> Option<Result<(String, String), String>> {
    let normalized = input.to_ascii_lowercase();
    let attack_word = ["attack", "hit", "strike", "fight", "shoot"]
        .iter()
        .any(|word| {
            normalized
                .split(|c: char| !c.is_alphabetic())
                .any(|token| token == *word)
        });
    if !attack_word {
        return None;
    }
    let location = &state.current_location_id;
    let (actor_id, actor_name, actor_state) = runtime_actor_named(state, location, &normalized)?;
    if !actor_state.is_combatant() {
        return Some(Err(format!(
            "{} cannot engage enemies: its combat capability is unavailable.",
            actor_name
        )));
    }
    let target = first_matching_hostile(state, bundleless_target_query(input), location)
        .or_else(|| first_matching_hostile(state, None, location));
    Some(
        target
            .ok_or_else(|| "No valid hostile target is present for that actor action.".to_owned())
            .map(|target| (actor_id, target)),
    )
}

fn bundleless_target_query(input: &str) -> Option<&str> {
    // The target resolver also accepts a missing query as the current hostile focus. Keeping the
    // raw instruction here makes references such as "Lira, hit that one" deterministic.
    Some(input)
}

pub(super) fn first_matching_hostile(
    state: &RunState,
    query: Option<&str>,
    location: &str,
) -> Option<String> {
    let matches = |id: &str, name: &str| {
        query.is_none_or(|query| {
            let lowered = query.to_ascii_lowercase();
            lowered.contains(&name.to_ascii_lowercase())
                || lowered.contains(&id.to_ascii_lowercase())
        })
    };
    state
        .location_enemies
        .get(location)
        .and_then(|ids| {
            ids.iter()
                .find(|id| {
                    state.enemies_alive.contains(*id)
                        && state
                            .runtime_actor(id)
                            .is_some_and(|actor| actor.actor.has_health_contract())
                        && {
                            let name = state
                                .enemy_instances
                                .get(*id)
                                .map(|entry| entry.name.as_str())
                                .unwrap_or(id);
                            matches(id, name)
                        }
                })
                .cloned()
        })
        .or_else(|| {
            state.location_bosses.get(location).and_then(|ids| {
                ids.iter()
                    .find(|id| {
                        state.bosses_alive.contains(*id)
                            && state
                                .runtime_actor(id)
                                .is_some_and(|actor| actor.actor.has_health_contract())
                            && {
                                let name = state
                                    .boss_instances
                                    .get(*id)
                                    .map(|entry| entry.name.as_str())
                                    .unwrap_or(id);
                                matches(id, name)
                            }
                    })
                    .cloned()
            })
        })
        .or_else(|| {
            state.location_npcs.get(location).and_then(|npcs| {
                npcs.iter()
                    .find(|npc| {
                        npc.lifecycle == InstanceLifecycle::Active
                            && npc.disposition == ActorDisposition::Hostile
                            && npc.actor.has_health_contract()
                            && matches(&npc.instance_id, &npc.name)
                    })
                    .map(|npc| npc.instance_id.clone())
            })
        })
        .or_else(|| {
            state.location_items.get(location).and_then(|item_ids| {
                item_ids
                    .iter()
                    .filter_map(|item_id| state.world_item_instances.get(item_id))
                    .find(|item| {
                        item.lifecycle == InstanceLifecycle::Active
                            && item.disposition == ActorDisposition::Hostile
                            && item
                                .actor
                                .as_ref()
                                .is_some_and(|actor| actor.has_health_contract())
                            && matches(
                                if item.instance_id.is_empty() {
                                    item.id.as_str()
                                } else {
                                    item.instance_id.as_str()
                                },
                                &item.name,
                            )
                    })
                    .map(|item| {
                        if item.instance_id.is_empty() {
                            item.id.clone()
                        } else {
                            item.instance_id.clone()
                        }
                    })
            })
        })
}

/// Target tags are resolved once from the same runtime/template view used to select a hostile.
/// Generic status contracts can therefore modify an actor's attack without knowing whether the
/// target happens to be an enemy, boss, NPC, or actor-capable item.
pub(super) fn hostile_target_tags(
    state: &RunState,
    bundle: &DatapackBundle,
    target_id: &str,
) -> Vec<String> {
    state
        .runtime_actor(target_id)
        .map(|actor| actor.tags)
        .or_else(|| find_enemy(bundle, target_id).map(|enemy| enemy.tags.clone()))
        .or_else(|| find_boss(bundle, target_id).map(|boss| boss.tags.clone()))
        .or_else(|| {
            state
                .location_npcs
                .values()
                .flatten()
                .find_map(|npc| (npc.instance_id == target_id).then(|| npc.tags.clone()))
        })
        .or_else(|| {
            state
                .world_item_instances
                .get(target_id)
                .map(|item| item.tags.clone())
        })
        .unwrap_or_default()
}

/// Shared runtime actor combat path. The origin is an entity instance, not an NPC concept: any
/// materialized actor that carries the combat + health contract can enter this reducer path.
pub(super) fn handle_actor_attack(
    state: &mut RunState,
    bundle: &DatapackBundle,
    origin_id: &str,
    target_query: &str,
) -> ActionOutcome {
    if origin_id == state.player_entity_id && origin_id == "player" {
        // Player combat still uses the compatibility resolver for equipped damage, scenario
        // retaliation, and existing saves. It is reached through the same actor dispatcher as
        // every other origin, rather than being a separate action family.
        return handle_player_attack(
            state,
            bundle,
            (!target_query.trim().is_empty()).then_some(target_query),
        );
    }
    let location = state.current_location_id.clone();
    let Some((_id, origin_name, origin_actor)) = runtime_actor_by_id(state, &location, origin_id)
    else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Actor is not active in the current location.".to_owned(),
            }],
            lines: vec!["That actor cannot act here.".to_owned()],
        };
    };
    if !origin_actor.is_combatant() {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Actor lacks the combat capability contract.".to_owned(),
            }],
            lines: vec![format!("{} cannot engage enemies.", origin_name)],
        };
    }
    let Some(target_id) = first_matching_hostile(state, Some(target_query), &location) else {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "No valid hostile target is present.".to_owned(),
            }],
            lines: vec![format!("{} has no valid hostile target.", origin_name)],
        };
    };
    let target_tags = hostile_target_tags(state, bundle, &target_id);
    let context_tags = find_location(bundle, &location)
        .map(|location| location.tags.clone())
        .unwrap_or_default();
    let effects = evaluate_status_effects(
        state,
        origin_id,
        EffectTrigger::Combat,
        &target_tags,
        &context_tags,
    );
    let damage = (origin_actor
        .damage
        .expect("combat contract requires damage")
        + effects.additive_damage)
        .max(1);
    let (target_kind, target_name, defeated) =
        match damage_hostile_target(state, &target_id, damage) {
            Ok(result) => result,
            Err(reason) => return rejected_attack(reason),
        };
    resolved_actor_attack_outcome(
        origin_id,
        &origin_name,
        (&target_id, target_kind, &target_name),
        damage,
        defeated,
        &effects,
    )
}

pub(super) fn damage_hostile_target(
    state: &mut RunState,
    target_id: &str,
    damage: i32,
) -> Result<(EncounterKind, String, bool), String> {
    let actor = state
        .runtime_actor(target_id)
        .ok_or_else(|| "Target is no longer an active runtime actor.".to_owned())?;
    if !actor.actor.has_health_contract() {
        return Err("Target lacks the health capability contract.".to_owned());
    }
    let kind = if state.enemies_alive.contains(target_id) {
        EncounterKind::Enemy
    } else if state.bosses_alive.contains(target_id) {
        EncounterKind::Boss
    } else if state
        .location_npcs
        .values()
        .flatten()
        .any(|npc| npc.instance_id == target_id)
    {
        EncounterKind::Npc
    } else {
        EncounterKind::Actor
    };
    let name = actor.name;
    let (_, _, remaining_hp, _) = state.damage_runtime_actor_health(target_id, damage)?;
    let defeated = remaining_hp <= 0;
    if !defeated {
        return Ok((kind, name, false));
    }
    match kind {
        EncounterKind::Enemy => {
            if let Some(instance) = state.enemy_instances.get_mut(target_id) {
                instance.lifecycle = InstanceLifecycle::Removed;
                instance.bucket = super::state::EncounterBucket::Defeated;
            }
            state.enemies_alive.remove(target_id);
            state.enemies_defeated.insert(target_id.to_owned());
            state.actor_noise_targets.remove(target_id);
            state.actor_origins.remove(target_id);
            state.actor_searching.remove(target_id);
            state.actor_sight_targets.remove(target_id);
            state.actor_sight_subjects.remove(target_id);
            state.actor_sight_delays.remove(target_id);
            for entries in state.location_enemies.values_mut() {
                entries.retain(|entry| entry != target_id);
            }
        }
        EncounterKind::Boss => {
            if let Some(instance) = state.boss_instances.get_mut(target_id) {
                instance.lifecycle = InstanceLifecycle::Removed;
                instance.bucket = super::state::EncounterBucket::Defeated;
            }
            state.bosses_alive.remove(target_id);
            state.bosses_defeated.insert(target_id.to_owned());
            for entries in state.location_bosses.values_mut() {
                entries.retain(|entry| entry != target_id);
            }
        }
        EncounterKind::Npc => {
            for npcs in state.location_npcs.values_mut() {
                if let Some(npc) = npcs.iter_mut().find(|npc| npc.instance_id == target_id) {
                    npc.lifecycle = InstanceLifecycle::Removed;
                    break;
                }
            }
        }
        EncounterKind::Actor => {
            if let Some(item) = state.world_item_instances.get_mut(target_id) {
                item.lifecycle = InstanceLifecycle::Removed;
            }
            for entries in state.location_items.values_mut() {
                entries.retain(|entry| entry != target_id);
            }
        }
    }
    Ok((kind, name, true))
}

fn rejected_attack(reason: String) -> ActionOutcome {
    ActionOutcome {
        events: vec![GameEvent::ActionRejected {
            reason: reason.clone(),
        }],
        lines: vec![reason],
    }
}

fn resolved_actor_attack_outcome(
    origin_id: &str,
    origin_name: &str,
    target: (&str, super::actions::EncounterKind, &str),
    damage: i32,
    defeated: bool,
    effects: &EffectEvaluation,
) -> ActionOutcome {
    let (target_id, target_kind, target_name) = target;
    let mut events = vec![GameEvent::AttackResolved {
        origin_id: origin_id.to_owned(),
        target_id: target_id.to_owned(),
        target_kind,
        damage,
        defeated,
    }];
    events.extend(
        effects
            .expired_instance_ids
            .iter()
            .cloned()
            .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
    );
    let mut lines = vec![format!(
        "{} attacks {} for {} damage.",
        origin_name, target_name, damage
    )];
    if effects.additive_damage != 0 {
        lines.push(format!(
            "Active effects modify this attack by {:+} damage.",
            effects.additive_damage
        ));
    }
    ActionOutcome { events, lines }
}

/// Runs the small autonomous-combat slice of the shared actor substrate. A friendly entity is not
/// an NPC special case: any active runtime actor in the current location that has the full combat
/// contract may use the same `handle_actor_attack` reducer path during a host tick. Movement,
/// dialogue, and other autonomous policies remain separate capability contracts rather than being
/// implied by this combat capability.
pub(super) fn apply_friendly_actor_combat_turns(
    state: &mut RunState,
    bundle: &DatapackBundle,
    outcome: &mut ActionOutcome,
) {
    let location = state.current_location_id.clone();
    let actor_ids = state
        .runtime_actor_ids_at(&location)
        .into_iter()
        .filter(|actor_id| {
            state.runtime_actor(actor_id).is_some_and(|actor| {
                actor.disposition == ActorDisposition::Friendly && actor.actor.is_combatant()
            })
        })
        .collect::<Vec<_>>();

    for actor_id in actor_ids {
        if !state.actor_is_ready(&actor_id) {
            continue;
        }
        let Some(target_id) = first_matching_hostile(state, None, &location) else {
            break;
        };
        let context_tags = find_location(bundle, &location)
            .map(|location| location.tags.as_slice())
            .unwrap_or_default();
        let timing =
            evaluate_timing_effects(state, &actor_id, &[ActionTimingKind::Combat], context_tags);
        if timing.blocked_by_instance_id.is_some() {
            continue;
        }
        let actor_outcome = handle_actor_attack(state, bundle, &actor_id, &target_id);
        if actor_outcome.events.iter().any(|event| {
            matches!(event, GameEvent::AttackResolved { origin_id, .. } if origin_id == &actor_id)
        }) {
            let cost = adjusted_timing_cost(state.actor_attack_cost(&actor_id), timing.cost_delta);
            state.commit_actor_action_time(&actor_id, cost);
            outcome.events.extend(actor_outcome.events);
            outcome.lines.extend(actor_outcome.lines);
            outcome.events.extend(
                consume_timing_effects(state, &timing.consume_on_success)
                    .into_iter()
                    .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
            );
        }
    }
}

/// Common runtime lookup: entity nouns only choose their storage bucket, while capabilities
/// decide whether the returned entity may use a host action.
fn runtime_actor_by_id(
    state: &RunState,
    location: &str,
    origin_id: &str,
) -> Option<(String, String, super::state::ActorState)> {
    let actor = state.runtime_actor(origin_id)?;
    (actor.location_id == location).then_some((actor.entity_id, actor.name, actor.actor))
}

fn runtime_actor_named(
    state: &RunState,
    location: &str,
    instruction: &str,
) -> Option<(String, String, super::state::ActorState)> {
    // Encounter actors now share the runtime directory. Prefer a non-hostile named actor for a
    // command such as "attack shambler with Lira" so the target's name cannot accidentally win
    // merely because it sorts before the requested origin. Hostile actors remain commandable
    // when they are the only named match.
    state
        .runtime_actor_ids_at(location)
        .into_iter()
        .filter_map(|actor_id| state.runtime_actor(&actor_id))
        .filter(|actor| {
            actor.entity_id != state.player_entity_id
                && actor.location_id == location
                && instruction.contains(&actor.name.to_ascii_lowercase())
        })
        .min_by_key(|actor| matches!(actor.disposition, ActorDisposition::Hostile))
        .map(|actor| (actor.entity_id, actor.name, actor.actor))
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
