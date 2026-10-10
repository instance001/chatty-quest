use crate::data::datapacks::DatapackBundle;

use super::abilities::{
    award_progress, expire_ability_grants, grant_ability, handle_use_ability,
    learn_ability_from_item, unlock_ability,
};
use super::actions::{ActionOutcome, GameAction, GameEvent, TransferIntent};
use super::combat::{apply_friendly_actor_combat_turns, handle_actor_attack};
use super::effects::{
    adjusted_timing_cost, consume_timing_effects, evaluate_status_effects, evaluate_timing_effects,
};
use super::epilogue::apply_epilogue_action;
use super::inspect::handle_inspect;
use super::items::{handle_equip, handle_take, handle_transfer_item, handle_use};
use super::movement::{handle_barricade, handle_unlock};
use super::noise::apply_noise_for_action;
use super::objectives::{
    objective_condition_statuses, objective_progress_lines, update_objective_completion,
};
use super::queries::{describe_current_location, find_item};
use super::social::SocialEventKind;
use super::spawned_ai::{apply_autonomous_actor_turns, handle_actor_move};
use super::state::{
    ActionTimingKind, EffectDuration, EffectTrigger, InstanceLifecycle, InventoryEntry, ItemBucket,
    ItemOrigin, ItemOwner, NpcInstance, RunState, StatusOwner, TemporaryExitInstance,
};
use super::summary::append_rolling_summary;
use super::wait::handle_wait;

#[cfg(test)]
use super::spawned_ai::{
    SpawnedEnemyStep, deterministic_hazard_break_roll, spawned_enemy_next_step,
};

pub fn apply_action(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: GameAction,
) -> ActionOutcome {
    // Zero HP is reversible incapacity, not a terminal run state. A downed controller can only
    // advance world time; any future revival must come from independent host/world behavior.
    if !state.controlled_actor_is_alive() && !matches!(action, GameAction::Wait) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "The controlled actor is incapacitated; only wait is available.".to_owned(),
            }],
            lines: vec![
                "The controlled actor is incapacitated. Wait for the world to advance.".to_owned(),
            ],
        };
    }

    if let Some(origin_id) = time_consuming_action_origin(state, &action)
        && !matches!(&action, GameAction::Wait)
        && state.runtime_actor_has_tag(&origin_id, "dead")
    {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: format!("{origin_id} is incapacitated and cannot act."),
            }],
            lines: vec!["An incapacitated actor cannot take that action.".to_owned()],
        };
    }

    if state.active_objective.completed {
        let action_for_timing = action.clone();
        let origin_id = time_consuming_action_origin(state, &action_for_timing)
            .expect("every accepted reducer action has an actor origin");
        if !state.actor_is_ready(&origin_id) {
            let remaining = state.actor_readiness_remaining(&origin_id);
            return ActionOutcome {
                events: vec![GameEvent::ActionRejected {
                    reason: format!(
                        "Actor is not ready for another action for {remaining} world-time unit(s)."
                    ),
                }],
                lines: vec![format!(
                    "That actor is not ready for another action for {remaining} world-time unit(s)."
                )],
            };
        }
        let timing_effects = timing_effects_for_action(state, bundle, &action_for_timing);
        if let Some(instance_id) = timing_effects.blocked_by_instance_id {
            return timing_blocked_outcome(&instance_id);
        }
        let mut outcome = apply_epilogue_action(state, bundle, action);
        if !action_was_rejected_or_blocked(&outcome) {
            let action_cost = adjusted_timing_cost(
                action_cost(state, bundle, &action_for_timing),
                timing_effects.cost_delta,
            );
            state.commit_actor_action_time(&origin_id, action_cost);
            advance_turn_index_by(state, action_cost);
            outcome.events.extend(
                consume_timing_effects(state, &timing_effects.consume_on_success)
                    .into_iter()
                    .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
            );
            outcome.events.extend(expire_ability_grants(state));
        }
        append_rolling_summary(state, &outcome);
        return outcome;
    }

    let was_alive = state.controlled_actor_is_alive();
    if let Some(origin_id) = time_consuming_action_origin(state, &action)
        && !state.actor_is_ready(&origin_id)
    {
        let remaining = state.actor_readiness_remaining(&origin_id);
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: format!(
                    "Actor is not ready for another action for {remaining} world-time unit(s)."
                ),
            }],
            lines: vec![format!(
                "That actor is not ready for another action for {remaining} world-time unit(s)."
            )],
        };
    }
    let action_for_noise = action.clone();
    let timing_effects = timing_effects_for_action(state, bundle, &action_for_noise);
    if let Some(instance_id) = timing_effects.blocked_by_instance_id.as_deref() {
        return timing_blocked_outcome(instance_id);
    }
    let objective_before = objective_condition_statuses(state);
    let mut outcome = match action {
        GameAction::Help => ActionOutcome {
            events: vec![GameEvent::HelpShown],
            lines: vec![
                "Commands: help, look, go <location>, unlock <location>, barricade <location>, inspect <thing>, take <item>, drop <item>, equip <item>, use <item>, attack [target], wait. You can also say: give <item> to <actor>, <actor> take my <item>, ask a named actor to give an item it owns, steal <item> from <actor>, cast <ability> at <target>, learn <ability>, learn <carried scroll>, help/insult/threaten/betray a named actor, ask a combat-capable actor to attack <target>, ask a grant-owning actor to cast <ability> at <target>, or ask a mobile actor to go <location>."
                    .to_owned(),
            ],
        },
        GameAction::Look => ActionOutcome {
            events: vec![GameEvent::LocationLooked {
                origin_id: state.player_entity_id.clone(),
                location_id: state.current_location_id.clone(),
            }],
            lines: describe_current_location(state, bundle),
        },
        GameAction::Move { destination } => {
            let player_entity_id = state.player_entity_id.clone();
            handle_actor_move(state, bundle, &player_entity_id, &destination, None)
        }
        GameAction::Unlock { target } => handle_unlock(state, bundle, &target),
        GameAction::Barricade { target } => handle_barricade(state, bundle, &target),
        GameAction::Inspect { target } => handle_inspect(state, bundle, &target),
        GameAction::Take { item_name } => handle_take(state, bundle, &item_name),
        GameAction::Equip { item_name } => handle_equip(state, &item_name),
        GameAction::Use { item_name } => handle_use(state, bundle, &item_name),
        GameAction::Attack => {
            let player_entity_id = state.player_entity_id.clone();
            handle_actor_attack(state, bundle, &player_entity_id, "")
        }
        GameAction::AttackTarget { target } => {
            let player_entity_id = state.player_entity_id.clone();
            handle_actor_attack(state, bundle, &player_entity_id, &target)
        }
        GameAction::ActorAttack { origin_id, target } => {
            handle_actor_attack(state, bundle, &origin_id, &target)
        }
        GameAction::ActorMove {
            origin_id,
            destination,
        } => handle_actor_move(state, bundle, &origin_id, &destination, None),
        GameAction::TransferItem {
            item_id,
            origin,
            destination,
            intent,
        } => {
            let mut outcome = match validate_player_actor_transfer(state, &origin, &destination, intent) {
                Ok(()) => handle_transfer_item(
                    state,
                    bundle,
                    &item_id,
                    origin.clone(),
                    destination.clone(),
                ),
                Err(reason) => ActionOutcome {
                    events: vec![GameEvent::ActionRejected {
                        reason: reason.clone(),
                    }],
                    lines: vec![reason],
                },
            };
            apply_transfer_social_event(state, &origin, &destination, intent, &mut outcome);
            outcome
        }
        GameAction::ApplySocialEvent {
            origin_id,
            subject_id,
            kind,
        } => apply_social_event(state, origin_id, subject_id, kind),
        GameAction::UseAbility {
            origin_id,
            grant_id,
            target,
        } => handle_use_ability(state, bundle, &origin_id, &grant_id, &target),
        GameAction::UnlockAbility { owner, template_id } => {
            unlock_ability(state, bundle, owner, &template_id)
        }
        GameAction::GrantAbility {
            owner,
            template_id,
            remaining_uses,
            duration_turns,
            source,
        } => grant_ability(
            state,
            bundle,
            owner,
            &template_id,
            remaining_uses,
            duration_turns,
            source,
        ),
        GameAction::LearnAbilityFromItem {
            learner_id,
            item_id,
            grant_id,
        } => learn_ability_from_item(state, bundle, &learner_id, &item_id, &grant_id),
        GameAction::AwardProgress {
            recipient_id,
            pool,
            amount,
        } => award_progress(state, bundle, &recipient_id, &pool, amount),
        GameAction::Wait => handle_wait(state, bundle),
        GameAction::AdmitNarratorItem {
            name,
            description,
            damage,
            condition,
            tags,
            actor,
            disposition,
        } => admit_narrator_item(state, name, description, damage, condition, tags, actor, disposition),
        GameAction::AdmitNarratorStatus {
            name,
            trigger,
            duration,
            additive_damage,
            additive_healing,
            required_target_tags,
            required_context_tags,
        } => admit_narrator_status(
            state,
            name,
            trigger,
            duration,
            additive_damage,
            additive_healing,
            required_target_tags,
            required_context_tags,
        ),
        GameAction::AdmitNarratorNpc {
            name,
            description,
            faction,
            tags,
            actor,
            disposition,
        } => admit_narrator_npc(state, name, description, faction, tags, actor, disposition),
        GameAction::AdmitNarratorEnemy {
            name,
            description,
            max_hp,
            damage,
            tags,
        } => admit_narrator_enemy(state, name, description, max_hp, damage, tags),
        GameAction::AdmitNarratorBoss { name, description, max_hp, damage, tags } => {
            admit_narrator_boss(state, name, description, max_hp, damage, tags)
        }
        GameAction::AdmitNarratorObjective { name, description, completion_key } => {
            admit_narrator_objective(state, bundle, name, description, completion_key)
        }
        GameAction::AdmitNarratorTemporaryExit {
            name,
            description,
            from_location_id,
            to_location_id,
            locked,
        } => admit_narrator_temporary_exit(
            state,
            bundle,
            name,
            description,
            from_location_id,
            to_location_id,
            locked,
        ),
    };

    let accepted_turn = !action_was_rejected_or_blocked(&outcome);
    apply_action_effects(
        state,
        bundle,
        &action_for_noise,
        action_succeeded(&outcome),
        &mut outcome,
    );
    if accepted_turn {
        let action_cost = adjusted_timing_cost(
            action_cost(state, bundle, &action_for_noise),
            timing_effects.cost_delta,
        );
        if let Some(origin_id) = time_consuming_action_origin(state, &action_for_noise) {
            state.commit_actor_action_time(&origin_id, action_cost);
        }
        advance_turn_index_by(state, action_cost);
        outcome.events.extend(
            consume_timing_effects(state, &timing_effects.consume_on_success)
                .into_iter()
                .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
        );
        outcome.events.extend(expire_ability_grants(state));
    }

    if accepted_turn {
        apply_noise_for_action(state, bundle, &action_for_noise, &mut outcome);
    }
    if accepted_turn {
        apply_autonomous_actor_turns(state, bundle, &mut outcome);
        if matches!(action_for_noise, GameAction::Wait) {
            apply_friendly_actor_combat_turns(state, bundle, &mut outcome);
        }
    }

    let objective_after = objective_condition_statuses(state);
    outcome.lines.extend(objective_progress_lines(
        &objective_before,
        &objective_after,
    ));

    // A lethal autonomous response can occur after the initiating action. Do not let the
    // objective resolver convert that same dead run into a victory.
    if state.controlled_actor_is_alive() && update_objective_completion(state) {
        outcome.events.push(GameEvent::ObjectiveCompleted {
            objective_id: state.active_objective.id.clone(),
        });
        outcome.events.push(GameEvent::RunWon);
        outcome.lines.push(format!(
            "Objective complete: {}.",
            state.active_objective.name
        ));
        outcome.lines.push("You win.".to_owned());
    }

    for objective in state.additional_objectives.iter_mut().filter(|objective| {
        !objective.completed
            && objective.required_location_id.as_deref() == Some(state.current_location_id.as_str())
    }) {
        objective.completed = true;
        objective.lifecycle = InstanceLifecycle::Completed;
        objective.bucket = super::state::ObjectiveBucket::Completed;
        outcome.events.push(GameEvent::ObjectiveCompleted {
            objective_id: objective.id.clone(),
        });
        outcome.lines.push(format!(
            "Additional objective complete: {}.",
            objective.name
        ));
    }

    if was_alive && !state.controlled_actor_is_alive() {
        outcome.lines.push(
            "You are incapacitated at 0 HP. You cannot act, but time may still pass and the world may yet provide revival."
                .to_owned(),
        );
    }

    append_rolling_summary(state, &outcome);
    ActionOutcome {
        events: outcome.events,
        lines: outcome.lines,
    }
}

/// A transfer can be mechanically meaningful to the recipient without creating a second
/// NPC/player "gift" pathway.  The item still moves through `TransferItem`; this merely records
/// the generic social consequence after the reducer has confirmed that move.
fn apply_transfer_social_event(
    state: &mut RunState,
    origin: &ItemOwner,
    destination: &ItemOwner,
    intent: TransferIntent,
    outcome: &mut ActionOutcome,
) {
    if !outcome
        .events
        .iter()
        .any(|event| matches!(event, GameEvent::ItemTransferred { .. }))
    {
        return;
    }
    let controlled_owner = state.controlled_inventory_owner();
    let (subject_id, kind) = match (intent, origin, destination) {
        (TransferIntent::Theft, ItemOwner::Entity(entity_id), destination)
            if *destination == controlled_owner =>
        {
            (entity_id, SocialEventKind::Theft)
        }
        (TransferIntent::Ordinary, origin, ItemOwner::Entity(entity_id))
            if *origin == controlled_owner =>
        {
            (entity_id, SocialEventKind::Gift)
        }
        (TransferIntent::Ordinary, ItemOwner::Entity(entity_id), destination)
            if *destination == controlled_owner =>
        {
            (entity_id, SocialEventKind::ReturnItem)
        }
        _ => return,
    };
    let Ok(social) = state.apply_social_event(subject_id, kind) else {
        // Transfer authority must remain independent from the optional social consumer. An
        // unsupported future endpoint can still own an item; it simply has no player relation.
        return;
    };
    outcome.events.push(GameEvent::SocialEventApplied {
        origin_id: state.player_entity_id.clone(),
        subject_id: subject_id.clone(),
        kind,
        relationship_delta: social.delta,
        relationship_score: social.relationship_score,
        disposition: social.disposition.clone(),
    });
    outcome.lines.push(format!(
        "Relationship with {subject_id}: {:+} (score {}; {:?}).",
        social.delta, social.relationship_score, social.disposition
    ));
}

/// Natural-language adapters are conveniences only. This reducer guard prevents a future UI or
/// caller from bypassing co-location and disposition rules with a direct `TransferItem` request.
fn validate_player_actor_transfer(
    state: &RunState,
    origin: &ItemOwner,
    destination: &ItemOwner,
    intent: TransferIntent,
) -> Result<(), String> {
    let controlled_owner = state.controlled_inventory_owner();
    let entity_id = match (origin, destination) {
        (origin, ItemOwner::Entity(entity_id)) if *origin == controlled_owner => entity_id,
        (ItemOwner::Entity(entity_id), destination) if *destination == controlled_owner => {
            entity_id
        }
        _ => return Ok(()),
    };
    if intent == TransferIntent::Theft
        && !matches!((origin, destination), (ItemOwner::Entity(_), destination) if *destination == controlled_owner)
    {
        return Err(
            "Theft must move an item from an entity owner to the controlled actor.".to_owned(),
        );
    }
    let actor = state.runtime_actor(entity_id).ok_or_else(|| {
        "That transfer endpoint is not an active actor at the current location.".to_owned()
    })?;
    if actor.location_id != state.current_location_id {
        return Err(format!(
            "{} is not at the current location, so the transfer cannot occur.",
            actor.name
        ));
    }
    if actor.disposition == crate::game::ActorDisposition::Hostile {
        return Err(format!(
            "{} is hostile and will not participate in an item transfer.",
            actor.name
        ));
    }
    Ok(())
}

fn apply_social_event(
    state: &mut RunState,
    origin_id: String,
    subject_id: String,
    kind: SocialEventKind,
) -> ActionOutcome {
    match state.apply_social_event(&subject_id, kind) {
        Ok(social) => ActionOutcome {
            events: vec![GameEvent::SocialEventApplied {
                origin_id,
                subject_id,
                kind,
                relationship_delta: social.delta,
                relationship_score: social.relationship_score,
                disposition: social.disposition.clone(),
            }],
            lines: vec![format!(
                "Relationship changed by {:+}; score {} is now {:?}.",
                social.delta, social.relationship_score, social.disposition
            )],
        },
        Err(reason) => ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        },
    }
}

fn apply_action_effects(
    state: &mut RunState,
    bundle: &DatapackBundle,
    action: &GameAction,
    accepted: bool,
    outcome: &mut ActionOutcome,
) {
    let trigger = match (action, accepted) {
        // Combat effects are resolved by the authoritative combat path, which has the target
        // tags needed to validate them. Movement, however, is fully origin-only and must use the
        // same generic status path whether it was requested as `Move` or `ActorMove`.
        (
            GameAction::Attack | GameAction::AttackTarget { .. } | GameAction::ActorAttack { .. },
            true,
        ) => return,
        (_, false) => EffectTrigger::Failure,
        (GameAction::Move { .. } | GameAction::ActorMove { .. }, true) => EffectTrigger::Movement,
        // The cast itself is ability-shaped, but a successfully resolved movement ability has
        // also performed the same movement trigger as an ordinary move. Combat abilities apply
        // their target-aware effects inside `handle_use_ability` and deliberately do not pass
        // through this origin-only path a second time.
        (GameAction::UseAbility { grant_id, .. }, true) => {
            let is_movement = state
                .ability_grants
                .iter()
                .find(|grant| grant.instance_id == *grant_id)
                .and_then(|grant| {
                    bundle
                        .abilities
                        .iter()
                        .find(|ability| ability.id == grant.template_id)
                })
                .is_some_and(|ability| {
                    matches!(
                        ability.effect,
                        Some(crate::data::datapacks::AbilityEffect::Move)
                    )
                });
            if !is_movement {
                return;
            }
            EffectTrigger::Movement
        }
        (GameAction::Inspect { .. } | GameAction::Look, true) => EffectTrigger::Inspect,
        (GameAction::Wait, true) => EffectTrigger::Wait,
        _ => return,
    };
    let origin_id = time_consuming_action_origin(state, action)
        .expect("every ordinary action has an actor origin");
    apply_actor_status_effects(state, bundle, &origin_id, trigger, &[], outcome);
    // Scene-duration ownership follows a real reducer-confirmed movement, not a command spelling.
    // This covers ordinary movement and movement abilities while avoiding an NPC's movement
    // accidentally clearing a player or another entity's personal scene-duration status.
    let movement = outcome.events.iter().find_map(|event| match event {
        GameEvent::Moved {
            from_location_id, ..
        } => time_consuming_action_origin(state, action)
            .map(|origin_id| (origin_id, from_location_id.clone())),
        GameEvent::ActorMoved {
            origin_id,
            from_location_id,
            ..
        } => Some((origin_id.clone(), from_location_id.clone())),
        _ => None,
    });
    if accepted && let Some((movement_origin, from_location_id)) = movement {
        expire_scene_duration_effects_after_movement(
            state,
            &movement_origin,
            &from_location_id,
            outcome,
        );
    }
}

/// Applies a reducer-confirmed status trigger to its actual actor origin. Autonomous turns,
/// direct actor actions, and controller-issued actions all call this one helper so a status is
/// never dependent on which UI or AI adapter requested the same host action.
pub(super) fn apply_actor_status_effects(
    state: &mut RunState,
    bundle: &DatapackBundle,
    origin_id: &str,
    trigger: EffectTrigger,
    target_tags: &[String],
    outcome: &mut ActionOutcome,
) {
    let context_tags = bundle
        .locations
        .iter()
        .find(|location| location.id == state.current_location_id)
        .map(|location| location.tags.clone())
        .unwrap_or_default();
    let effects = evaluate_status_effects(state, origin_id, trigger, target_tags, &context_tags);
    if effects.additive_healing != 0 {
        // Health belongs to the actor that took the action. `restore_runtime_actor_health`
        // keeps the legacy player fields compatible while handling NPCs and actor-items through
        // their canonical state, so a status never accidentally heals the player just because
        // another actor moved.
        if let Ok((_, applied, remaining_hp)) =
            state.restore_runtime_actor_health(origin_id, effects.additive_healing)
            && applied != 0
        {
            outcome.events.push(GameEvent::HealthRestored {
                origin_id: "host status system".to_owned(),
                target_id: origin_id.to_owned(),
                amount: applied,
                remaining_hp,
            });
            outcome
                .lines
                .push(format!("Active effects restore {} HP.", applied));
        }
    }
    outcome.events.extend(
        effects
            .expired_instance_ids
            .into_iter()
            .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
    );
}

/// Scene-duration statuses expire when, and only when, their owner actually changes scene. The
/// caller supplies the confirmed movement origin so autonomous movement shares the same rule.
pub(super) fn expire_scene_duration_effects_after_movement(
    state: &mut RunState,
    movement_origin: &str,
    from_location_id: &str,
    outcome: &mut ActionOutcome,
) {
    for effect in state.status_effects.iter_mut().filter(|effect| {
        if effect.lifecycle != InstanceLifecycle::Active || effect.duration != EffectDuration::Scene
        {
            return false;
        }
        match effect.owner {
            StatusOwner::Player => movement_origin == "player",
            StatusOwner::Entity => effect.owner_id.as_deref() == Some(movement_origin),
            StatusOwner::Scene => {
                movement_origin == state.player_entity_id
                    && effect
                        .owner_id
                        .as_deref()
                        .is_none_or(|location_id| location_id == from_location_id)
            }
        }
    }) {
        effect.lifecycle = InstanceLifecycle::Consumed;
        outcome.events.push(GameEvent::StatusEffectExpired {
            instance_id: effect.instance_id.clone(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn admit_narrator_item(
    state: &mut RunState,
    name: String,
    description: String,
    damage: i32,
    condition: Option<String>,
    tags: Vec<String>,
    actor: Option<super::state::ActorState>,
    disposition: super::state::ActorDisposition,
) -> ActionOutcome {
    let owner = state.controlled_inventory_owner();
    if !state.accepts_item_transfer(&owner) {
        let reason = "The controlled actor cannot carry narrator-admitted items.".to_owned();
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        };
    }
    state.scratch_item_admission_count = state.scratch_item_admission_count.saturating_add(1);
    let item_id = format!("narrator_item_{}", state.scratch_item_admission_count);
    let instance_id = format!("scratch_item_{}", state.scratch_item_admission_count);
    let Some(inventory) = state.entity_inventory_mut(&owner) else {
        let reason = "The controlled actor has no inventory endpoint.".to_owned();
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![reason],
        };
    };
    inventory.push(InventoryEntry {
        id: item_id.clone(),
        instance_id,
        template_id: item_id.clone(),
        bucket: ItemBucket::Inventory,
        location_id: None,
        lifecycle: InstanceLifecycle::Active,
        name: name.clone(),
        description,
        damage,
        origin: ItemOrigin::NarratorApproved,
        condition: condition.clone(),
        tags,
        actor,
        disposition,
        inventory: Vec::new(),
        equipped_item_id: None,
    });
    let condition_line = condition
        .map(|value| format!(" Condition: {}.", value))
        .unwrap_or_default();
    ActionOutcome {
        events: vec![GameEvent::NarratorItemAdmitted { item_id }],
        lines: vec![format!(
            "RD Engine admitted narrator proposal: {} is now real for this run.{}",
            name, condition_line
        )],
    }
}

#[allow(clippy::too_many_arguments)]
fn admit_narrator_status(
    state: &mut RunState,
    name: String,
    trigger: String,
    duration: String,
    additive_damage: i32,
    additive_healing: i32,
    required_target_tags: Vec<String>,
    required_context_tags: Vec<String>,
) -> ActionOutcome {
    if let Some(reason) = narrator_status_contract_error(
        &name,
        &trigger,
        &duration,
        additive_damage,
        additive_healing,
    ) {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: reason.clone(),
            }],
            lines: vec![format!("Narrator status proposal rejected: {reason}")],
        };
    }
    let admission_number = state.status_effects.len().saturating_add(1);
    let instance_id = format!("narrator_status_{admission_number}");
    let trigger = match trigger.trim().to_ascii_lowercase().as_str() {
        "movement" => EffectTrigger::Movement,
        "inspect" => EffectTrigger::Inspect,
        "wait" => EffectTrigger::Wait,
        "failure" => EffectTrigger::Failure,
        _ => EffectTrigger::Combat,
    };
    let duration = match duration.trim().to_ascii_lowercase().as_str() {
        "scene" => EffectDuration::Scene,
        "until_used" | "until-used" => EffectDuration::UntilUsed,
        _ => EffectDuration::Run,
    };
    state
        .status_effects
        .push(super::state::StatusEffectInstance {
            instance_id: instance_id.clone(),
            template_id: format!("scratch_status_{admission_number}"),
            owner: super::state::StatusOwner::Entity,
            owner_id: Some(state.player_entity_id.clone()),
            trigger,
            duration,
            additive_damage,
            additive_healing,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags,
            required_context_tags,
            lifecycle: InstanceLifecycle::Active,
        });
    ActionOutcome {
        events: vec![GameEvent::NarratorStatusAdmitted {
            instance_id: instance_id.clone(),
        }],
        lines: vec![format!(
            "RD Engine admitted narrator status proposal: {name} is now active for this run."
        )],
    }
}

/// The reducer is the final admission boundary. This prevents an approval-card edit or another
/// caller from materializing a status whose advertised mechanics cannot execute in the current
/// host effect substrate.
fn narrator_status_contract_error(
    name: &str,
    trigger: &str,
    duration: &str,
    additive_damage: i32,
    additive_healing: i32,
) -> Option<String> {
    let trigger = trigger.trim().to_ascii_lowercase();
    let duration = duration.trim().to_ascii_lowercase();
    if name.trim().is_empty() {
        return Some("status name is required".to_owned());
    }
    if !matches!(
        trigger.as_str(),
        "combat" | "movement" | "inspect" | "wait" | "failure"
    ) {
        return Some("status trigger is not supported by the host".to_owned());
    }
    if !matches!(
        duration.as_str(),
        "scene" | "run" | "until_used" | "until-used"
    ) {
        return Some("status duration is not supported by the host".to_owned());
    }
    if additive_damage == 0 && additive_healing == 0 {
        return Some("status has no reducer-backed effect".to_owned());
    }
    if trigger == "combat" && additive_healing != 0 {
        return Some("combat statuses currently support damage, not healing".to_owned());
    }
    if trigger != "combat" && additive_damage != 0 {
        return Some("non-combat statuses currently support healing, not damage".to_owned());
    }
    None
}

fn admit_narrator_npc(
    state: &mut RunState,
    name: String,
    description: String,
    faction: String,
    tags: Vec<String>,
    actor: super::state::ActorState,
    disposition: super::state::ActorDisposition,
) -> ActionOutcome {
    let admission_number = state
        .location_npcs
        .values()
        .map(Vec::len)
        .sum::<usize>()
        .saturating_add(1);
    let instance_id = format!("narrator_npc_{admission_number}");
    state
        .location_npcs
        .entry(state.current_location_id.clone())
        .or_default()
        .push(NpcInstance {
            instance_id: instance_id.clone(),
            template_id: format!("scratch_npc_{admission_number}"),
            location_id: state.current_location_id.clone(),
            lifecycle: InstanceLifecycle::Active,
            provenance: "narrator_approved".to_owned(),
            name: name.clone(),
            description,
            faction,
            tags,
            disposition,
            actor,
            quest_objective_ids: Vec::new(),
            inventory: Vec::new(),
            equipped_item_id: None,
        });
    ActionOutcome {
        events: vec![GameEvent::NarratorNpcAdmitted {
            instance_id: instance_id.clone(),
        }],
        lines: vec![format!(
            "RD Engine admitted narrator NPC proposal: {name} is now present here."
        )],
    }
}

fn admit_narrator_enemy(
    state: &mut RunState,
    name: String,
    description: String,
    max_hp: i32,
    damage: i32,
    tags: Vec<String>,
) -> ActionOutcome {
    let admission_number = state.enemy_instances.len().saturating_add(1);
    let enemy_id = format!("narrator_enemy_{admission_number}");
    state.enemies_alive.insert(enemy_id.clone());
    state.enemy_hp.insert(enemy_id.clone(), max_hp.max(1));
    state
        .location_enemies
        .entry(state.current_location_id.clone())
        .or_default()
        .push(enemy_id.clone());
    state.enemy_instances.insert(
        enemy_id.clone(),
        super::state::EncounterInstance {
            instance_id: enemy_id.clone(),
            template_id: format!("scratch_enemy_{admission_number}"),
            location_id: state.current_location_id.clone(),
            bucket: super::state::EncounterBucket::Live,
            lifecycle: InstanceLifecycle::Active,
            current_hp: max_hp.max(1),
            max_hp: max_hp.max(1),
            provenance: "narrator_approved".to_owned(),
            name: name.clone(),
            description,
            damage: damage.max(0),
            traversal_cost: 1,
            attack_cost: 1,
            tags,
            capabilities: vec!["health".to_owned(), "combat".to_owned()],
        },
    );
    ActionOutcome {
        events: vec![GameEvent::NarratorEnemyAdmitted {
            instance_id: enemy_id,
        }],
        lines: vec![format!(
            "RD Engine admitted narrator enemy proposal: {name} is now a live threat."
        )],
    }
}

fn admit_narrator_boss(
    state: &mut RunState,
    name: String,
    description: String,
    max_hp: i32,
    damage: i32,
    tags: Vec<String>,
) -> ActionOutcome {
    let number = state.boss_instances.len().saturating_add(1);
    let id = format!("narrator_boss_{number}");
    let hp = max_hp.max(1);
    state.bosses_alive.insert(id.clone());
    state.boss_hp.insert(id.clone(), hp);
    state
        .location_bosses
        .entry(state.current_location_id.clone())
        .or_default()
        .push(id.clone());
    state.boss_instances.insert(
        id.clone(),
        super::state::EncounterInstance {
            instance_id: id.clone(),
            template_id: format!("scratch_boss_{number}"),
            location_id: state.current_location_id.clone(),
            bucket: super::state::EncounterBucket::Live,
            lifecycle: InstanceLifecycle::Active,
            current_hp: hp,
            max_hp: hp,
            provenance: "narrator_approved".to_owned(),
            name: name.clone(),
            description,
            damage: damage.max(0),
            traversal_cost: 1,
            attack_cost: 1,
            tags,
            capabilities: vec!["health".to_owned(), "combat".to_owned()],
        },
    );
    ActionOutcome {
        events: vec![GameEvent::NarratorBossAdmitted { instance_id: id }],
        lines: vec![format!(
            "RD Engine admitted narrator boss proposal: {name} is now a live threat."
        )],
    }
}

fn admit_narrator_objective(
    state: &mut RunState,
    bundle: &DatapackBundle,
    name: String,
    description: String,
    completion_key: String,
) -> ActionOutcome {
    if !bundle
        .locations
        .iter()
        .any(|location| location.id == completion_key)
    {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: format!(
                    "Narrator objective completion location is unknown: {completion_key}."
                ),
            }],
            lines: vec![
                "RD Engine rejected narrator objective proposal: completion location is unknown."
                    .to_owned(),
            ],
        };
    }
    let number = state.additional_objectives.len().saturating_add(1);
    let id = format!("narrator_objective_{number}");
    state
        .additional_objectives
        .push(super::state::ObjectiveState {
            instance_id: id.clone(),
            template_id: format!("scratch_objective_{number}"),
            lifecycle: InstanceLifecycle::Active,
            bucket: super::state::ObjectiveBucket::Active,
            id: id.clone(),
            name: name.clone(),
            description,
            target_boss_id: None,
            required_item_id: None,
            required_location_id: Some(completion_key),
            completed: false,
        });
    ActionOutcome {
        events: Vec::new(),
        lines: vec![format!(
            "RD Engine admitted narrator objective proposal: {name} is now active."
        )],
    }
}

fn admit_narrator_temporary_exit(
    state: &mut RunState,
    bundle: &DatapackBundle,
    name: String,
    _description: String,
    from_location_id: String,
    to_location_id: String,
    locked: bool,
) -> ActionOutcome {
    let both_locations_exist = [from_location_id.as_str(), to_location_id.as_str()]
        .iter()
        .all(|id| bundle.locations.iter().any(|location| location.id == *id));
    if !both_locations_exist || from_location_id == to_location_id {
        return ActionOutcome {
            events: vec![GameEvent::ActionRejected {
                reason: "Narrator temporary exit endpoints are invalid.".to_owned(),
            }],
            lines: vec![
                "RD Engine rejected narrator temporary exit proposal: endpoints are invalid."
                    .to_owned(),
            ],
        };
    }
    let number = state.temporary_exits.len().saturating_add(1);
    let id = format!("narrator_exit_{number}");
    state.temporary_exits.push(TemporaryExitInstance {
        instance_id: id.clone(),
        template_id: format!("scratch_exit_{number}"),
        from_location_id,
        to_location_id,
        locked,
        lifecycle: InstanceLifecycle::Active,
        provenance: "narrator_approved".to_owned(),
    });
    ActionOutcome {
        events: vec![GameEvent::NarratorTemporaryExitAdmitted { instance_id: id }],
        lines: vec![format!(
            "RD Engine admitted narrator temporary exit proposal: {name} is now available."
        )],
    }
}

fn action_was_rejected_or_blocked(outcome: &ActionOutcome) -> bool {
    outcome.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ActionRejected { .. } | GameEvent::MovementBlocked { .. }
        )
    })
}

/// A reducer action may commit time without succeeding at its immediate purpose: swinging with no
/// valid target is the canonical example. Keep that distinction separate from whether a turn was
/// accepted, so a status that explicitly listens for `failure` can react to a whiff while world
/// time and readiness still advance normally.
fn action_succeeded(outcome: &ActionOutcome) -> bool {
    !outcome.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ActionRejected { .. }
                | GameEvent::MovementBlocked { .. }
                | GameEvent::AttackWhiff { .. }
        )
    })
}

fn advance_turn_index_by(state: &mut RunState, action_cost: u64) {
    state.turn_index = state.turn_index.saturating_add(action_cost.max(1));
}

fn timing_effects_for_action(
    state: &RunState,
    bundle: &DatapackBundle,
    action: &GameAction,
) -> super::effects::TimingEffectEvaluation {
    let context_tags = super::queries::find_location(bundle, &state.current_location_id)
        .map(|location| location.tags.as_slice())
        .unwrap_or_default();
    evaluate_timing_effects(
        state,
        &time_consuming_action_origin(state, action)
            .expect("every timed action has an actor origin"),
        &action_timing_kinds(state, bundle, action),
        context_tags,
    )
}

/// An action can have more than one semantic timing category. A cast is always an `ability`, but
/// a movement or damage effect is also a movement or combat action for generic status gates. This
/// keeps a rooted actor from bypassing a movement restriction by putting ordinary travel inside a
/// template, without teaching statuses about any particular spell or skill name.
fn action_timing_kinds(
    state: &RunState,
    bundle: &DatapackBundle,
    action: &GameAction,
) -> Vec<ActionTimingKind> {
    match action {
        GameAction::Move { .. } | GameAction::ActorMove { .. } => vec![ActionTimingKind::Movement],
        GameAction::Attack | GameAction::AttackTarget { .. } | GameAction::ActorAttack { .. } => {
            vec![ActionTimingKind::Combat]
        }
        GameAction::UseAbility { grant_id, .. } => {
            let mut kinds = vec![ActionTimingKind::Ability];
            let effect = state
                .ability_grants
                .iter()
                .find(|grant| grant.instance_id == *grant_id)
                .and_then(|grant| {
                    bundle
                        .abilities
                        .iter()
                        .find(|ability| ability.id == grant.template_id)
                })
                .and_then(|ability| ability.effect.as_ref());
            match effect {
                Some(crate::data::datapacks::AbilityEffect::Damage { .. }) => {
                    kinds.push(ActionTimingKind::Combat);
                }
                Some(crate::data::datapacks::AbilityEffect::Move) => {
                    kinds.push(ActionTimingKind::Movement);
                }
                _ => {}
            }
            kinds
        }
        GameAction::Wait => vec![ActionTimingKind::Wait],
        _ => vec![ActionTimingKind::General],
    }
}

fn timing_blocked_outcome(instance_id: &str) -> ActionOutcome {
    ActionOutcome {
        events: vec![GameEvent::ActionRejected {
            reason: format!("An active status effect '{instance_id}' blocks that action category."),
        }],
        lines: vec!["An active status effect prevents that action right now.".to_owned()],
    }
}

#[cfg(test)]
fn advance_turn_index(state: &mut RunState) {
    advance_turn_index_by(state, 1);
}

/// Every accepted reducer action already advances authoritative world time. Associate that same
/// commitment with an origin so an inventory, social, or traversal action cannot sidestep actor
/// readiness merely because it was introduced before the common timing primitive. Explicit actor
/// actions keep their own origin; player-issued actions use the currently controlled entity.
fn time_consuming_action_origin(state: &RunState, action: &GameAction) -> Option<String> {
    match action {
        GameAction::ActorAttack { origin_id, .. }
        | GameAction::ActorMove { origin_id, .. }
        | GameAction::UseAbility { origin_id, .. } => Some(origin_id.clone()),
        GameAction::ApplySocialEvent { origin_id, .. } => Some(origin_id.clone()),
        _ => Some(state.player_entity_id.clone()),
    }
}

/// Resolves a successful action's authored world-time cost. The controller is deliberately not a
/// branch in this function: actor/template/action data chooses the duration.
fn action_cost(state: &RunState, bundle: &DatapackBundle, action: &GameAction) -> u64 {
    let default = 1;
    match action {
        GameAction::Move { .. } => actor_traversal_cost(state, &state.player_entity_id),
        GameAction::ActorMove { origin_id, .. } => actor_traversal_cost(state, origin_id),
        GameAction::Attack | GameAction::AttackTarget { .. } => {
            controlled_attack_cost(state, bundle)
        }
        GameAction::ActorAttack { origin_id, .. } => actor_attack_cost(state, origin_id),
        GameAction::UseAbility { grant_id, .. } => state
            .ability_grants
            .iter()
            .find(|grant| grant.instance_id == *grant_id)
            .and_then(|grant| {
                bundle
                    .abilities
                    .iter()
                    .find(|ability| ability.id == grant.template_id)
            })
            .map_or(default, |ability| ability.action_cost.max(1)),
        _ => default,
    }
}

fn actor_traversal_cost(state: &RunState, actor_id: &str) -> u64 {
    state.actor_traversal_cost(actor_id)
}

fn actor_attack_cost(state: &RunState, actor_id: &str) -> u64 {
    state.actor_attack_cost(actor_id)
}

fn controlled_attack_cost(state: &RunState, bundle: &DatapackBundle) -> u64 {
    let owner = state.controlled_inventory_owner();
    state
        .equipped_item_id_for_owner(&owner)
        .and_then(|equipped| {
            state
                .entity_inventory(&owner)
                .into_iter()
                .flatten()
                .find(|item| item.id == equipped || item.instance_id == equipped)
        })
        .and_then(|item| find_item(bundle, item.effective_template_id()))
        .map_or_else(
            || actor_attack_cost(state, &state.player_entity_id),
            |item| item.attack_cost.max(1),
        )
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::actions::{
        EncounterKind, GameAction, GameEvent, ItemUseEffect, MovementHazardKind, TransferIntent,
    };
    use crate::game::generation::generate_new_run;
    use crate::game::social::SocialEventKind;
    use crate::game::state::{
        ActionTimingKind, ActorDisposition, ActorState, EffectDuration, EffectTrigger,
        InstanceLifecycle, ItemOwner, NpcInstance, RunState, StatusEffectInstance, StatusOwner,
    };

    use super::{
        SpawnedEnemyStep, advance_turn_index, apply_action, deterministic_hazard_break_roll,
        spawned_enemy_next_step,
    };
    use crate::game::summary::ROLLING_SUMMARY_LIMIT;

    #[test]
    fn admitted_narrator_status_becomes_a_structured_active_effect() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorStatus {
                name: "Emergency Focus".to_owned(),
                trigger: "inspect".to_owned(),
                duration: "until_used".to_owned(),
                additive_damage: 0,
                additive_healing: 2,
                required_target_tags: Vec::new(),
                required_context_tags: Vec::new(),
            },
        );
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::NarratorStatusAdmitted { instance_id }] if instance_id == "narrator_status_1"
        ));
        assert_eq!(state.status_effects.len(), 1);
        assert_eq!(state.status_effects[0].trigger, EffectTrigger::Inspect);
        assert_eq!(state.status_effects[0].duration, EffectDuration::UntilUsed);
        assert_eq!(state.status_effects[0].additive_healing, 2);
    }

    #[test]
    fn narrator_status_admission_rejects_a_decorative_no_op() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorStatus {
                name: "Empty Omen".to_owned(),
                trigger: "wait".to_owned(),
                duration: "run".to_owned(),
                additive_damage: 0,
                additive_healing: 0,
                required_target_tags: Vec::new(),
                required_context_tags: Vec::new(),
            },
        );
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("no reducer-backed effect")
        ));
        assert!(state.status_effects.is_empty());
    }

    #[test]
    fn admitted_narrator_npc_enters_the_current_location_bucket() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Mara".to_owned(),
                description: "A watchful survivor.".to_owned(),
                faction: "survivors".to_owned(),
                tags: vec!["guide".to_owned()],
                actor: crate::game::ActorState::default(),
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::NarratorNpcAdmitted { instance_id }] if instance_id.starts_with("narrator_npc_")
        ));
        let npcs = state
            .location_npcs
            .get(&location_id)
            .expect("NPC bucket exists");
        assert_eq!(npcs[0].name, "Mara");
        assert_eq!(npcs[0].faction, "survivors");
        assert!(
            crate::game::queries::describe_current_location(&state, &bundle)
                .iter()
                .any(|line| line.contains("People here: Mara [survivors; Neutral]"))
        );
    }

    #[test]
    fn generic_transfer_moves_one_existing_item_between_player_and_inventory_actor() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Carrier".to_owned(),
                description: "Carries things.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let actor_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!(),
        };
        let item_id = state.inventory[0].id.clone();
        state.equipped_item_id = Some(item_id.clone());
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: crate::game::state::ItemOwner::Player,
                destination: crate::game::state::ItemOwner::Entity(actor_id.clone()),
                intent: TransferIntent::Ordinary,
            },
        );
        assert!(matches!(
            outcome.events.first(),
            Some(GameEvent::ItemTransferred { .. })
        ));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SocialEventApplied { kind: crate::game::social::SocialEventKind::Gift, subject_id, .. }
                if subject_id == &actor_id
        )));
        assert!(!state.inventory.iter().any(|item| item.id == item_id));
        assert!(state.equipped_item_id.is_none());
        assert!(
            state
                .location_npcs
                .values()
                .flatten()
                .find(|npc| npc.instance_id == actor_id)
                .is_some_and(|npc| npc.inventory.iter().any(|item| item.id == item_id))
        );
    }

    #[test]
    fn controlled_inventory_actor_uses_the_same_transfer_social_policy() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let admit_inventory_actor = |state: &mut RunState, name: &str| {
            let outcome = apply_action(
                state,
                &bundle,
                GameAction::AdmitNarratorNpc {
                    name: name.to_owned(),
                    description: "A transfer fixture.".to_owned(),
                    faction: "test".to_owned(),
                    tags: Vec::new(),
                    actor: crate::game::ActorState {
                        capabilities: vec!["inventory".to_owned()],
                        ..Default::default()
                    },
                    disposition: crate::game::ActorDisposition::Friendly,
                },
            );
            match outcome.events.as_slice() {
                [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
                other => panic!("unexpected actor admission events: {other:?}"),
            }
        };
        let controller_id = admit_inventory_actor(&mut state, "Controller");
        let recipient_id = admit_inventory_actor(&mut state, "Recipient");
        let item_id = state.inventory[0].id.clone();

        let stow = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: ItemOwner::Player,
                destination: ItemOwner::Entity(controller_id.clone()),
                intent: TransferIntent::Ordinary,
            },
        );
        assert!(matches!(
            stow.events.first(),
            Some(GameEvent::ItemTransferred { .. })
        ));
        state.player_entity_id = controller_id.clone();

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: ItemOwner::Entity(controller_id.clone()),
                destination: ItemOwner::Entity(recipient_id.clone()),
                intent: TransferIntent::Ordinary,
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ItemTransferred { origin: ItemOwner::Entity(origin), destination: ItemOwner::Entity(destination), .. }, GameEvent::SocialEventApplied { subject_id, kind: SocialEventKind::Gift, .. }]
                if origin == &controller_id && destination == &recipient_id && subject_id == &recipient_id
        ));
        assert!(
            state
                .entity_inventory(&ItemOwner::Entity(recipient_id))
                .is_some_and(|inventory| inventory.iter().any(|item| item.id == item_id))
        );
    }

    #[test]
    fn theft_uses_transfer_item_and_emits_a_confirmed_social_consequence() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Mark".to_owned(),
                description: "A transfer-policy fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["trusting".to_owned()],
                actor: crate::game::ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        let actor_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!(),
        };
        let item_id = state.inventory[0].id.clone();
        let stow = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: ItemOwner::Player,
                destination: ItemOwner::Entity(actor_id.clone()),
                intent: TransferIntent::Ordinary,
            },
        );
        assert!(matches!(
            stow.events.first(),
            Some(GameEvent::ItemTransferred { .. })
        ));

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: ItemOwner::Entity(actor_id.clone()),
                destination: ItemOwner::Player,
                intent: TransferIntent::Theft,
            },
        );

        assert!(matches!(
            outcome.events.first(),
            Some(GameEvent::ItemTransferred {
                item_id: transferred,
                origin: ItemOwner::Entity(origin),
                destination: ItemOwner::Player,
            }) if transferred == &item_id && origin == &actor_id
        ));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SocialEventApplied {
                subject_id,
                kind: SocialEventKind::Theft,
                relationship_delta,
                ..
            } if subject_id == &actor_id && *relationship_delta < 0
        )));
        assert!(state.inventory.iter().any(|item| item.id == item_id));
        assert!(
            state
                .entity_inventory(&ItemOwner::Entity(actor_id.clone()))
                .expect("actor inventory")
                .iter()
                .all(|item| item.id != item_id)
        );
        assert!(
            state
                .social_state(&actor_id)
                .is_some_and(|social| social.relationship_score < 0)
        );
    }

    #[test]
    fn direct_transfer_cannot_bypass_hostile_actor_refusal() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Hostile carrier".to_owned(),
                description: "A transfer validation fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState {
                    capabilities: vec!["inventory".to_owned()],
                    ..Default::default()
                },
                disposition: crate::game::ActorDisposition::Hostile,
            },
        );
        let actor_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!(),
        };
        let item_id = state.inventory[0].id.clone();
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: crate::game::state::ItemOwner::Player,
                destination: crate::game::state::ItemOwner::Entity(actor_id),
                intent: TransferIntent::Ordinary,
            },
        );
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("hostile")
        ));
        assert!(state.inventory.iter().any(|item| item.id == item_id));
    }

    #[test]
    fn generic_social_action_is_reducer_backed_and_changes_disposition() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Social fixture".to_owned(),
                description: "Tests shared social state.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["trusting".to_owned()],
                actor: crate::game::ActorState::default(),
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        let actor_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!(),
        };
        let player_id = state.player_entity_id.clone();
        for _ in 0..2 {
            let outcome = apply_action(
                &mut state,
                &bundle,
                GameAction::ApplySocialEvent {
                    origin_id: player_id.clone(),
                    subject_id: actor_id.clone(),
                    kind: crate::game::social::SocialEventKind::Gift,
                },
            );
            assert!(outcome.events.iter().any(|event| matches!(
                event,
                GameEvent::SocialEventApplied { subject_id, .. } if subject_id == &actor_id
            )));
        }
        assert_eq!(
            state.runtime_actor(&actor_id).expect("actor").disposition,
            crate::game::ActorDisposition::Friendly
        );
    }

    #[test]
    fn generic_transfer_can_drop_an_owned_item_into_the_current_location() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let item_id = state.inventory[0].id.clone();
        let location_id = state.current_location_id.clone();

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: crate::game::state::ItemOwner::Player,
                destination: crate::game::state::ItemOwner::Location(location_id.clone()),
                intent: TransferIntent::Ordinary,
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ItemTransferred { .. }]
        ));
        assert!(!state.inventory.iter().any(|item| item.id == item_id));
        assert!(
            state.location_items[&location_id]
                .iter()
                .any(|instance_id| {
                    state
                        .world_item_instances
                        .get(instance_id)
                        .is_some_and(|item| {
                            item.id == item_id && item.location_id.as_deref() == Some(&location_id)
                        })
                })
        );
    }

    #[test]
    fn admitted_actor_npc_uses_generic_combat_and_emits_its_own_origin() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Lira Voss".to_owned(),
                description: "A capable ally.".to_owned(),
                faction: "survivors".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(
                    Some(10),
                    Some(5),
                    true,
                    true,
                    true,
                    true,
                ),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];
        state.noise_level = 0;
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: origin_id.clone(),
                target: target_id.clone(),
            },
        );
        assert_eq!(state.enemy_hp[&target_id], (hp_before - 5).max(0));
        assert!(outcome.events.iter().any(|event| matches!(event,
            GameEvent::AttackResolved { origin_id: actual, target_id: target, damage: 5, .. }
            if actual == &origin_id && target == &target_id
        )));
        assert_eq!(state.noise_level, 1);
        assert!(
            outcome
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );
        assert!(!state.enemies_alive.contains(&target_id));
        assert!(!state.location_enemies[&state.current_location_id].contains(&target_id));
    }

    #[test]
    fn friendly_actor_with_combat_contract_uses_the_shared_combat_path_on_a_wait_tick() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Tick ally".to_owned(),
                description: "A friendly autonomous combat fixture.".to_owned(),
                faction: "survivors".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(
                    Some(10),
                    Some(5),
                    false,
                    false,
                    false,
                    true,
                ),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];

        let outcome = apply_action(&mut state, &bundle, GameAction::Wait);

        assert_eq!(state.enemy_hp[&target_id], (hp_before - 5).max(0));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                origin_id: actual_origin,
                target_id: actual_target,
                damage: 5,
                ..
            } if actual_origin == &origin_id && actual_target == &target_id
        )));
    }

    #[test]
    fn friendly_actor_without_combat_contract_does_not_act_on_a_wait_tick() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Peaceful witness".to_owned(),
                description: "A non-combat autonomous fixture.".to_owned(),
                faction: "survivors".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::default(),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];

        let outcome = apply_action(&mut state, &bundle, GameAction::Wait);

        assert_eq!(state.enemy_hp[&target_id], hp_before);
        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { origin_id: actual_origin, .. } if actual_origin == &origin_id
        )));
    }

    #[test]
    fn friendly_actor_capable_item_uses_the_same_autonomous_combat_tick() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorItem {
                name: "Autonomous fixture".to_owned(),
                description: "A friendly actor-capable item.".to_owned(),
                damage: 0,
                condition: None,
                tags: Vec::new(),
                actor: Some(crate::game::ActorState::configured(
                    Some(10),
                    Some(4),
                    false,
                    false,
                    false,
                    true,
                )),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let item_id = match admission.events.as_slice() {
            [GameEvent::NarratorItemAdmitted { item_id }] => item_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let location_id = state.current_location_id.clone();
        apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: ItemOwner::Player,
                destination: ItemOwner::Location(location_id),
                intent: TransferIntent::Ordinary,
            },
        );
        let item_instance_id = state
            .world_item_instances
            .values()
            .find(|item| item.id == item_id)
            .map(|item| item.instance_id.clone())
            .expect("transferred actor item enters the world bucket");
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];

        let outcome = apply_action(&mut state, &bundle, GameAction::Wait);

        assert_eq!(state.enemy_hp[&target_id], hp_before - 4);
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { origin_id, target_id: actual_target, damage: 4, .. }
                if origin_id == &item_instance_id && actual_target == &target_id
        )));
    }

    #[test]
    fn carried_actor_item_can_use_the_same_reducer_backed_combat_path() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorItem {
                name: "Carried combat fixture".to_owned(),
                description: "An actor item kept in the player's inventory.".to_owned(),
                damage: 0,
                condition: None,
                tags: vec!["item".to_owned(), "combat".to_owned()],
                actor: Some(crate::game::ActorState::configured(
                    Some(8),
                    Some(4),
                    false,
                    false,
                    false,
                    true,
                )),
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        let item_id = match admission.events.as_slice() {
            [GameEvent::NarratorItemAdmitted { item_id }] => item_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let item_instance_id = state
            .inventory
            .iter()
            .find(|item| item.id == item_id)
            .map(|item| item.instance_id.clone())
            .expect("admitted item remains in player inventory");
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: item_instance_id.clone(),
                target: target_id.clone(),
            },
        );

        assert_eq!(state.enemy_hp[&target_id], hp_before - 4);
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { origin_id, target_id: actual_target, damage: 4, .. }
                if origin_id == &item_instance_id && actual_target == &target_id
        )));
    }

    #[test]
    fn controlled_actor_with_movement_contract_uses_the_ordinary_move_path() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let from_location_id = state.current_location_id.clone();
        let destination = bundle
            .locations
            .iter()
            .find(|location| location.id == from_location_id)
            .and_then(|location| location.connections.first())
            .cloned()
            .expect("fixture start location has an exit");
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Mobile fixture".to_owned(),
                description: "A shared actor movement fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(None, None, false, false, true, false),
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        // `Move` belongs to the currently controlled actor, irrespective of its noun. This is
        // deliberately not the explicit `ActorMove` path: it proves the normal player command
        // reaches the same reducer-backed movement contract when control is transferred.
        state.player_entity_id = origin_id.clone();

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: destination.clone(),
            },
        );

        assert_eq!(state.current_location_id, destination);
        assert!(
            state
                .location_npcs
                .get(&destination)
                .is_some_and(|npcs| npcs.iter().any(|npc| npc.instance_id == origin_id))
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                from_location_id: from,
                to_location_id: to,
                ..
            } if moved_id == &origin_id && from == &from_location_id && to == &destination
        )));
        assert!(
            crate::game::action_traces(
                &GameAction::Move {
                    destination: destination.clone(),
                },
                &outcome,
            )
            .iter()
            .any(|trace| {
                trace.action_type == "movement"
                    && trace.origin == origin_id
                    && trace.impact == destination
                    && trace.outcome == "moved"
            })
        );
    }

    #[test]
    fn controller_issued_wait_look_and_inspect_keep_the_controlled_actor_as_origin() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Observer fixture".to_owned(),
                description: "A controller-origin fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(
                    Some(10),
                    None,
                    false,
                    false,
                    false,
                    false,
                ),
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        state.player_entity_id = origin_id.clone();

        for action in [
            GameAction::Wait,
            GameAction::Look,
            GameAction::Inspect {
                target: "room".to_owned(),
            },
        ] {
            let outcome = apply_action(&mut state, &bundle, action.clone());
            assert!(
                crate::game::action_traces(&action, &outcome)
                    .iter()
                    .any(|trace| trace.origin == origin_id)
            );
        }
    }

    #[test]
    fn actor_move_applies_the_moving_actors_status_effects() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let destination = bundle
            .locations
            .iter()
            .find(|location| location.id == state.current_location_id)
            .and_then(|location| location.connections.first())
            .cloned()
            .expect("fixture start location has an exit");
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Restoring walker".to_owned(),
                description: "A movement-status fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: ActorState::configured(Some(5), None, false, false, true, false),
                disposition: ActorDisposition::Neutral,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        state
            .location_npcs
            .get_mut(&state.current_location_id)
            .expect("admitted NPC remains at the starting location")[0]
            .actor
            .current_hp = Some(2);
        state.status_effects.push(StatusEffectInstance {
            instance_id: "walking_restoration".to_owned(),
            template_id: "walking_restoration".to_owned(),
            owner: StatusOwner::Entity,
            owner_id: Some(origin_id.clone()),
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 2,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorMove {
                origin_id: origin_id.clone(),
                destination,
            },
        );

        let moved = state
            .location_npcs
            .values()
            .flatten()
            .find(|npc| npc.instance_id == origin_id)
            .expect("NPC survives and moves");
        assert_eq!(moved.actor.current_hp, Some(4));
        assert_eq!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "walking_restoration"
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::HealthRestored {
                origin_id: event_origin,
                target_id,
                amount: 2,
                remaining_hp: 4,
            } if event_origin == "host status system" && target_id == &origin_id
        )));
    }

    #[test]
    fn actor_without_combat_contract_cannot_use_generic_combat() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Lira Voss".to_owned(),
                description: "A witness.".to_owned(),
                faction: "survivors".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::default(),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let origin_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!(),
        };
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id,
                target: target_id.clone(),
            },
        );
        assert_eq!(state.enemy_hp[&target_id], hp_before);
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { .. }]
        ));
    }

    #[test]
    fn combat_capable_runtime_actor_can_damage_a_hostile_runtime_actor() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let attacker = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Friendly fixture".to_owned(),
                description: "A combat-capable actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(
                    Some(10),
                    Some(3),
                    false,
                    false,
                    false,
                    true,
                ),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let attacker_id = match attacker.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let defender = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Hostile fixture".to_owned(),
                description: "A valid hostile actor target.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(
                    Some(8),
                    Some(1),
                    false,
                    false,
                    false,
                    true,
                ),
                disposition: crate::game::ActorDisposition::Hostile,
            },
        );
        let defender_id = match defender.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: attacker_id.clone(),
                target: "Hostile fixture".to_owned(),
            },
        );

        let defender = state
            .location_npcs
            .get(&state.current_location_id)
            .and_then(|npcs| npcs.iter().find(|npc| npc.instance_id == defender_id))
            .expect("hostile runtime actor remains in the location");
        assert_eq!(defender.actor.current_hp, Some(5));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                origin_id,
                target_id,
                target_kind: EncounterKind::Npc,
                damage: 3,
                defeated: false,
            } if origin_id == &attacker_id && target_id == &defender_id
        )));
    }

    #[test]
    fn combat_capable_runtime_actor_and_player_can_damage_a_hostile_actor_item() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let attacker = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Friendly fixture".to_owned(),
                description: "A combat-capable actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState::configured(
                    Some(10),
                    Some(3),
                    false,
                    false,
                    false,
                    true,
                ),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let attacker_id = match attacker.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let item = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorItem {
                name: "Hostile fixture item".to_owned(),
                description: "A valid hostile actor-item target.".to_owned(),
                damage: 0,
                condition: None,
                tags: Vec::new(),
                actor: Some(crate::game::ActorState::configured(
                    Some(4),
                    Some(1),
                    false,
                    false,
                    false,
                    true,
                )),
                disposition: crate::game::ActorDisposition::Hostile,
            },
        );
        let item_id = match item.events.as_slice() {
            [GameEvent::NarratorItemAdmitted { item_id }] => item_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let location_id = state.current_location_id.clone();
        apply_action(
            &mut state,
            &bundle,
            GameAction::TransferItem {
                item_id: item_id.clone(),
                origin: ItemOwner::Player,
                destination: ItemOwner::Location(location_id),
                intent: TransferIntent::Ordinary,
            },
        );
        let item_instance_id = state
            .world_item_instances
            .values()
            .find(|item| item.id == item_id)
            .map(|item| item.instance_id.clone())
            .expect("admitted actor item enters the world bucket");

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: attacker_id.clone(),
                target: "Hostile fixture item".to_owned(),
            },
        );

        let item = state
            .world_item_instances
            .get(&item_instance_id)
            .expect("runtime actor item remains inspectable");
        assert_eq!(
            item.actor.as_ref().and_then(|actor| actor.current_hp),
            Some(1)
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                origin_id,
                target_id,
                target_kind: EncounterKind::Actor,
                damage: 3,
                defeated: false,
            } if origin_id == &attacker_id && target_id == &item_instance_id
        )));

        // The player still enters through compatibility decoration, but actor-items must not be
        // an NPC-only target class. A targeted player action must select this item even while an
        // authored encounter is present, then use the same shared defeat cleanup.
        let player_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: "player".to_owned(),
                target: "Hostile fixture item".to_owned(),
            },
        );
        assert!(player_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                origin_id,
                target_id,
                target_kind: EncounterKind::Actor,
                damage: 1,
                defeated: true,
            } if origin_id == "player" && target_id == &item_instance_id
        )));
        assert_eq!(
            state.world_item_instances[&item_instance_id].lifecycle,
            InstanceLifecycle::Removed
        );
        assert!(
            state
                .location_items
                .values()
                .all(|item_ids| !item_ids.contains(&item_instance_id))
        );
    }

    #[test]
    fn targeted_attack_uses_the_current_controlled_actor_not_a_literal_player_id() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Controller fixture".to_owned(),
                description: "A combat-capable controlled actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: ActorState::configured(Some(10), Some(4), false, false, false, true),
                disposition: ActorDisposition::Friendly,
            },
        );
        let controller_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            other => panic!("unexpected admission events: {other:?}"),
        };
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        state.player_entity_id = controller_id.clone();

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AttackTarget {
                target: target_id.clone(),
            },
        );

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                origin_id,
                target_id: actual_target,
                damage: 4,
                ..
            } if origin_id == &controller_id && actual_target == &target_id
        )));
    }

    #[test]
    fn authored_friendly_npc_with_perception_is_not_a_hostile_combat_target() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        let npc = &state.location_npcs["kitchen"][0];
        assert!(npc.actor.can_hear && npc.actor.can_see);
        assert_eq!(npc.disposition, crate::game::ActorDisposition::Friendly);
        let hp_before = state.hp;
        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AttackWhiff { .. }))
        );
        assert_eq!(state.hp, hp_before);
    }

    #[test]
    fn hostile_combat_capable_npc_uses_shared_actor_health_and_death_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        let npc = &mut state
            .location_npcs
            .get_mut("kitchen")
            .expect("NPC placement")[0];
        npc.disposition = crate::game::ActorDisposition::Hostile;
        npc.actor = crate::game::ActorState::configured(Some(2), Some(1), true, true, false, true);
        let hp_before = state.hp;
        let first = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(first.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                target_kind: EncounterKind::Npc,
                defeated: false,
                ..
            }
        )));
        assert_eq!(state.hp, hp_before - 1);
        let second = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(second.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                target_kind: EncounterKind::Npc,
                defeated: true,
                ..
            }
        )));
        assert_eq!(
            state.location_npcs["kitchen"][0].lifecycle,
            InstanceLifecycle::Removed
        );
    }

    #[test]
    fn admitted_narrator_enemy_is_visible_and_uses_instance_combat_stats() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state
            .location_enemies
            .insert(state.current_location_id.clone(), Vec::new());
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorEnemy {
                name: "Fog Hound".to_owned(),
                description: "A test threat.".to_owned(),
                max_hp: 3,
                damage: 2,
                tags: vec!["hound".to_owned()],
            },
        );
        let enemy_id = admission
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::NarratorEnemyAdmitted { instance_id } => Some(instance_id.clone()),
                _ => None,
            })
            .expect("enemy admission event");
        assert!(
            crate::game::queries::describe_current_location(&state, &bundle)
                .iter()
                .any(|line| line.contains("Enemies here: Fog Hound."))
        );
        let hp_before = state.hp;
        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { target_id, target_kind: EncounterKind::Enemy, .. }
                if target_id == &enemy_id
        )));
        assert_eq!(state.hp, hp_before - 2);
        let admitted = state
            .enemy_instances
            .get(&enemy_id)
            .expect("admitted encounter remains inspectable");
        assert_eq!(admitted.description, "A test threat.");
        assert_eq!(admitted.tags, vec!["hound"]);
    }

    #[test]
    fn explicit_health_only_encounter_can_be_hit_but_cannot_retaliate() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = state.location_enemies[&state.current_location_id][0].clone();
        state
            .enemy_instances
            .get_mut(&enemy_id)
            .expect("fixture encounter")
            .capabilities = vec!["health".to_owned()];
        let hp_before = state.hp;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { target_id, .. } if target_id == &enemy_id
        )));
        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken { origin_id, .. } if origin_id == &enemy_id
        )));
        assert_eq!(state.hp, hp_before);
    }

    #[test]
    fn encounter_retaliation_uses_the_live_actor_damage_contract() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = state.location_enemies[&state.current_location_id][0].clone();
        state
            .enemy_instances
            .get_mut(&enemy_id)
            .expect("fixture encounter")
            .damage = 6;
        let hp_before = state.hp;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken { origin_id, amount: 6, .. } if origin_id == &enemy_id
        )));
        assert_eq!(state.hp, hp_before - 6);
    }

    #[test]
    fn admitted_narrator_boss_creates_a_live_runtime_encounter() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state
            .location_bosses
            .insert(state.current_location_id.clone(), Vec::new());
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorBoss {
                name: "Fog King".to_owned(),
                description: "A test boss.".to_owned(),
                max_hp: 12,
                damage: 3,
                tags: vec!["fog".to_owned()],
            },
        );
        let id = outcome
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::NarratorBossAdmitted { instance_id } => Some(instance_id),
                _ => None,
            })
            .expect("boss admission event");
        let boss = state.boss_instances.get(id).expect("boss instance");
        assert_eq!(boss.max_hp, 12);
        assert_eq!(boss.damage, 3);
        assert_eq!(boss.description, "A test boss.");
        assert_eq!(boss.tags, vec!["fog"]);
    }

    #[test]
    fn admitted_narrator_objective_stays_separate_from_the_primary_objective() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let primary_objective_id = state.active_objective.id.clone();

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorObjective {
                name: "Reach the laundry".to_owned(),
                description: "Find the hidden laundry route.".to_owned(),
                completion_key: "laundry".to_owned(),
            },
        );

        assert!(outcome.events.is_empty());
        assert_eq!(state.active_objective.id, primary_objective_id);
        assert_eq!(state.additional_objectives.len(), 1);
        let objective = &state.additional_objectives[0];
        assert_eq!(objective.name, "Reach the laundry");
        assert_eq!(objective.required_location_id.as_deref(), Some("laundry"));
        assert!(!objective.completed);
    }

    #[test]
    fn narrator_objective_rejects_an_unknown_completion_location() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorObjective {
                name: "Impossible errand".to_owned(),
                description: "This should not be admitted.".to_owned(),
                completion_key: "not_a_location".to_owned(),
            },
        );
        assert!(state.additional_objectives.is_empty());
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { .. }]
        ));
    }

    #[test]
    fn narrator_temporary_exit_admission_creates_a_live_route() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let from_location_id = state.current_location_id.clone();
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorTemporaryExit {
                name: "Service hatch".to_owned(),
                description: "A narrow route.".to_owned(),
                from_location_id: from_location_id.clone(),
                to_location_id: "laundry".to_owned(),
                locked: false,
            },
        );
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::NarratorTemporaryExitAdmitted { .. }]
        ));
        assert!(state.temporary_exits.iter().any(|exit| {
            exit.from_location_id == from_location_id && exit.to_location_id == "laundry"
        }));
    }

    #[test]
    fn runtime_tracking_actor_joins_the_host_noise_loop_without_a_spawn_id_prefix() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle");
        let mut state = generate_new_run(&bundle).state;
        let from_location_id = "kitchen".to_owned();
        let actor_id = "runtime_noise_tracker".to_owned();
        state.location_npcs.insert(
            from_location_id.clone(),
            vec![NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "runtime_noise_tracker".to_owned(),
                location_id: from_location_id.clone(),
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Noise tracker".to_owned(),
                description: "A generic tracking actor fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Neutral,
                actor: ActorState {
                    can_hear: true,
                    can_move: true,
                    capabilities: vec![
                        "hearing".to_owned(),
                        "movement".to_owned(),
                        "noise_track".to_owned(),
                    ],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(
            state
                .location_npcs
                .get(&state.current_location_id)
                .is_some_and(|npcs| npcs.iter().any(|npc| npc.instance_id == actor_id))
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::NoiseAttractorShifted { actor_ids, .. } if actor_ids.contains(&actor_id)
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved { origin_id, from_location_id: from, .. }
                if origin_id == &actor_id && from == &from_location_id
        )));
    }

    #[test]
    fn narrator_temporary_exit_rejects_unknown_endpoints() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorTemporaryExit {
                name: "Nowhere route".to_owned(),
                description: "Invalid route.".to_owned(),
                from_location_id: "not_a_location".to_owned(),
                to_location_id: "laundry".to_owned(),
                locked: false,
            },
        );
        assert!(state.temporary_exits.is_empty());
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { .. }]
        ));
    }

    #[test]
    fn active_temporary_exit_allows_anotherwise_unconnected_move() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state
            .temporary_exits
            .push(super::super::state::TemporaryExitInstance {
                instance_id: "test_exit".to_owned(),
                template_id: "test.exit".to_owned(),
                from_location_id: state.current_location_id.clone(),
                to_location_id: "laundry".to_owned(),
                locked: false,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
            });
        state
            .additional_objectives
            .push(super::super::state::ObjectiveState {
                instance_id: "test_objective".to_owned(),
                template_id: "test.objective".to_owned(),
                lifecycle: InstanceLifecycle::Active,
                bucket: super::super::state::ObjectiveBucket::Active,
                id: "test.objective".to_owned(),
                name: "Reach the laundry".to_owned(),
                description: "A test objective.".to_owned(),
                target_boss_id: None,
                required_item_id: None,
                required_location_id: Some("laundry".to_owned()),
                completed: false,
            });
        assert!(
            crate::game::queries::describe_current_location(&state, &bundle)
                .iter()
                .any(|line| line.contains("Temporary routes: Laundry [temporary]."))
        );
        assert!(
            crate::game::queries::describe_current_location(&state, &bundle)
                .iter()
                .any(|line| line.contains("Additional objectives: Reach the laundry."))
        );
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "laundry");
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::Moved { to_location_id, .. } if to_location_id == "laundry"
        )));
        assert!(state.additional_objectives[0].completed);
        assert_eq!(
            state.additional_objectives[0].lifecycle,
            InstanceLifecycle::Completed
        );
        assert!(!state.active_objective.completed);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::RunWon))
        );
    }

    #[test]
    fn locked_temporary_exit_blocks_the_route() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state
            .temporary_exits
            .push(super::super::state::TemporaryExitInstance {
                instance_id: "locked_exit".to_owned(),
                template_id: "test.locked_exit".to_owned(),
                from_location_id: state.current_location_id.clone(),
                to_location_id: "laundry".to_owned(),
                locked: true,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
            });
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "front_verandah");
        assert!(
            outcome
                .lines
                .iter()
                .any(|line| line == "That temporary route is locked.")
        );
    }

    #[test]
    fn invalid_movement_is_blocked_by_boundary_rules() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        assert_eq!(state.current_location_id, "front_verandah");
        assert!(matches!(
            outcome.events.first(),
            Some(GameEvent::MovementBlocked { attempted_destination, .. }) if attempted_destination == "laundry"
        ));
        assert_eq!(
            outcome.lines.first().map(String::as_str),
            Some("You make it three fences before the horde eats you, idiot.")
        );
    }

    #[test]
    fn accepted_actions_advance_turn_index_but_blocked_actions_do_not() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let look = apply_action(&mut state, &bundle, GameAction::Look);
        assert!(look.events.iter().any(|event| {
            matches!(event, GameEvent::LocationLooked { location_id, .. } if location_id == "front_verandah")
        }));
        assert_eq!(state.turn_index, 1);

        let blocked = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        assert!(
            blocked
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::MovementBlocked { .. }))
        );
        assert_eq!(state.turn_index, 1);
    }

    #[test]
    fn narrator_item_admission_creates_saveable_session_inventory_truth() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorItem {
                name: "A Very Serious Spoon".to_owned(),
                description: "It has made peace with being a weapon.".to_owned(),
                damage: 2,
                condition: None,
                tags: vec!["weapon".to_owned(), "silly".to_owned()],
                actor: Some(crate::game::ActorState::configured(
                    Some(4),
                    Some(1),
                    true,
                    false,
                    true,
                    true,
                )),
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );

        let item = state
            .inventory
            .iter()
            .find(|item| item.id == "narrator_item_1")
            .expect("admitted narrator item should enter inventory");
        assert_eq!(item.name, "A Very Serious Spoon");
        assert_eq!(item.damage, 2);
        let actor = item
            .actor
            .as_ref()
            .expect("approved actor envelope persists");
        assert!(actor.is_combatant());
        assert!(actor.can_hear && actor.can_move);
        assert_eq!(item.disposition, crate::game::ActorDisposition::Friendly);
        assert!(matches!(
            item.origin,
            super::super::state::ItemOrigin::NarratorApproved
        ));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::NarratorItemAdmitted { item_id } if item_id == "narrator_item_1"
        )));
    }

    #[test]
    fn station_smoke_test_pack_can_generate_move_fight_and_win() {
        let bundle = load_datapack_bundle_by_folder("station_smoke_test")
            .expect("expected station_smoke_test bundle to load");
        let mut state = generate_new_run(&bundle).state;

        assert_eq!(state.datapack_id, "station_smoke_test");
        assert_eq!(state.current_location_id, "station_platform");
        assert_eq!(
            state.active_objective.required_item_id.as_deref(),
            Some("brass_token")
        );

        let take = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "brass_token".to_owned(),
            },
        );
        assert!(take.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::ItemTransferred { item_id, origin: crate::game::state::ItemOwner::Location(_), destination: crate::game::state::ItemOwner::Player }
                    if item_id == "brass_token"
            )
        }));

        let move_to_signal_box = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "signal_box".to_owned(),
            },
        );
        assert!(move_to_signal_box.events.iter().any(|event| {
            matches!(event, GameEvent::Moved { to_location_id, .. } if to_location_id == "signal_box")
        }));

        let clear_guard = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(clear_guard.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AttackResolved {
                    target_id,
                    target_kind: EncounterKind::Enemy,
                    defeated: true,
                    ..
                } if target_id == "static_guard"
            )
        }));

        let hit_wraith = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(hit_wraith.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AttackResolved {
                    target_id,
                    target_kind: EncounterKind::Boss,
                    defeated: false,
                    ..
                } if target_id == "ticket_wraith"
            )
        }));

        let defeat_wraith = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(defeat_wraith.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AttackResolved {
                    target_id,
                    target_kind: EncounterKind::Boss,
                    defeated: true,
                    ..
                } if target_id == "ticket_wraith"
            )
        }));
        assert!(
            defeat_wraith
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
        assert!(
            defeat_wraith
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::RunWon))
        );
        assert!(state.active_objective.completed);
    }

    #[test]
    fn deterministic_rolls_advance_after_rolling_summary_is_capped() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_spawn_count = 1;
        state.noise_level = 3;
        state.rolling_summary = (0..ROLLING_SUMMARY_LIMIT)
            .map(|index| format!("summary {}", index))
            .collect();
        state.turn_index = ROLLING_SUMMARY_LIMIT as u64;

        let first_roll =
            deterministic_hazard_break_roll(&state, "noise_spawn_1_crawler_in_weeds", "garage");
        advance_turn_index(&mut state);
        let second_roll =
            deterministic_hazard_break_roll(&state, "noise_spawn_1_crawler_in_weeds", "garage");

        assert_eq!(state.rolling_summary.len(), ROLLING_SUMMARY_LIMIT);
        assert_ne!(first_roll, second_roll);
    }

    #[test]
    fn take_equip_and_use_update_structured_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        let take_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "medkit".to_owned(),
            },
        );
        assert!(state.inventory.iter().any(|item| item.id == "medkit"));
        assert!(
            state.location_items["kitchen"]
                .iter()
                .all(|item_id| item_id != "medkit")
        );
        assert!(matches!(
            take_outcome.events.as_slice(),
            [GameEvent::ItemTransferred { item_id, origin: crate::game::state::ItemOwner::Location(_), destination: crate::game::state::ItemOwner::Player }] if item_id == "medkit"
        ));
        assert!(
            !take_outcome
                .lines
                .iter()
                .any(|line| line.starts_with("Objective progress:"))
        );

        let equip_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "cricket_bat".to_owned(),
            },
        );
        assert_eq!(state.equipped_item_id.as_deref(), Some("cricket_bat"));
        assert!(matches!(
            equip_outcome.events.first(),
            Some(GameEvent::ItemEquipped { item_id, .. }) if item_id == "cricket_bat"
        ));

        state.hp = 6;
        let use_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "medkit".to_owned(),
            },
        );
        assert_eq!(state.hp, 10);
        assert!(!state.inventory.iter().any(|item| item.id == "medkit"));
        assert!(matches!(
            use_outcome.events.first(),
            Some(GameEvent::ItemUsed {
                item_id,
                effect: ItemUseEffect::Healing { amount: 4 },
                ..
            }) if item_id == "medkit"
        ));
        assert!(use_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::HealthRestored {
                origin_id,
                target_id,
                amount: 4,
                remaining_hp: 10,
            } if origin_id == "player" && target_id == "player"
        )));
    }

    #[test]
    fn ordinary_healing_item_cannot_revive_an_incapacitated_controller() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.inventory.push(crate::game::state::InventoryEntry {
            id: "medkit".to_owned(),
            instance_id: "test_medkit".to_owned(),
            template_id: "medkit".to_owned(),
            bucket: crate::game::state::ItemBucket::Inventory,
            location_id: None,
            lifecycle: crate::game::state::InstanceLifecycle::Active,
            name: "Medkit".to_owned(),
            description: "A test ordinary healing item.".to_owned(),
            damage: 0,
            origin: crate::game::state::ItemOrigin::Authored,
            condition: None,
            tags: Vec::new(),
            actor: None,
            disposition: crate::game::ActorDisposition::Neutral,
            inventory: Vec::new(),
            equipped_item_id: None,
        });
        state.hp = 0;

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "medkit".to_owned(),
            },
        );

        assert_eq!(state.hp, 0);
        assert!(state.inventory.iter().any(|item| item.id == "medkit"));
        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("incapacitated")
        ));
    }

    #[test]
    fn controlled_inventory_actor_uses_the_same_take_equip_and_use_path() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "kitchen".to_owned();
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Field medic".to_owned(),
                description: "A controlled inventory fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState {
                    current_hp: Some(6),
                    max_hp: Some(10),
                    capabilities: vec!["health".to_owned(), "inventory".to_owned()],
                    ..Default::default()
                },
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let actor_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!("NPC admission must identify the actor"),
        };
        state.player_entity_id = actor_id.clone();

        let admitted_item = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorItem {
                name: "Controller-owned fixture".to_owned(),
                description: "Must enter the controlled inventory endpoint.".to_owned(),
                damage: 0,
                condition: None,
                tags: Vec::new(),
                actor: None,
                disposition: crate::game::ActorDisposition::Neutral,
            },
        );
        let admitted_item_id = match admitted_item.events.as_slice() {
            [GameEvent::NarratorItemAdmitted { item_id }] => item_id.clone(),
            other => panic!("unexpected narrator item admission events: {other:?}"),
        };
        assert!(
            state
                .inventory
                .iter()
                .all(|item| item.id != admitted_item_id)
        );
        assert!(
            state
                .entity_inventory(&ItemOwner::Entity(actor_id.clone()))
                .is_some_and(|inventory| inventory.iter().any(|item| item.id == admitted_item_id))
        );
        assert!(matches!(
            state.resolve_player_drop_to_location(&format!("drop {admitted_item_id}")),
            Some(Ok((item_id, ItemOwner::Entity(origin), ItemOwner::Location(_))))
                if item_id == admitted_item_id && origin == actor_id
        ));

        let take = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "medkit".to_owned(),
            },
        );
        assert!(matches!(
            take.events.first(),
            Some(GameEvent::ItemTransferred {
                item_id,
                origin: ItemOwner::Location(_),
                destination: ItemOwner::Entity(destination),
            }) if item_id == "medkit" && destination == &actor_id
        ));
        assert!(
            state
                .entity_inventory(&ItemOwner::Entity(actor_id.clone()))
                .is_some_and(|inventory| inventory.iter().any(|item| item.id == "medkit"))
        );

        let equip = apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "medkit".to_owned(),
            },
        );
        assert!(matches!(
            equip.events.first(),
            Some(GameEvent::ItemEquipped { origin_id, item_id })
                if origin_id == &actor_id && item_id == "medkit"
        ));
        assert_eq!(
            state.equipped_item_id_for_owner(&ItemOwner::Entity(actor_id.clone())),
            Some("medkit")
        );

        let used = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "medkit".to_owned(),
            },
        );
        assert!(matches!(
            used.events.first(),
            Some(GameEvent::ItemUsed {
                origin_id,
                item_id,
                effect: ItemUseEffect::Healing { amount: 4 },
            }) if origin_id == &actor_id && item_id == "medkit"
        ));
        assert!(used.events.iter().any(|event| matches!(
            event,
            GameEvent::HealthRestored {
                origin_id,
                target_id,
                amount: 4,
                remaining_hp: 10,
            } if origin_id == &actor_id && target_id == &actor_id
        )));
        assert!(
            state
                .entity_inventory(&ItemOwner::Entity(actor_id))
                .is_some_and(|inventory| inventory.iter().all(|item| item.id != "medkit"))
        );
    }

    #[test]
    fn passive_pressure_incapacitates_the_controlled_runtime_actor_without_ending_the_run() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let admission = apply_action(
            &mut state,
            &bundle,
            GameAction::AdmitNarratorNpc {
                name: "Fragile controller".to_owned(),
                description: "A controlled health fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                actor: crate::game::ActorState {
                    current_hp: Some(1),
                    max_hp: Some(1),
                    capabilities: vec!["health".to_owned()],
                    ..Default::default()
                },
                disposition: crate::game::ActorDisposition::Friendly,
            },
        );
        let actor_id = match admission.events.as_slice() {
            [GameEvent::NarratorNpcAdmitted { instance_id }] => instance_id.clone(),
            _ => unreachable!("NPC admission must identify the actor"),
        };
        state.player_entity_id = actor_id.clone();
        let legacy_hp_before = state.hp;

        let outcome = apply_action(&mut state, &bundle, GameAction::Wait);

        assert_eq!(state.hp, legacy_hp_before);
        assert_eq!(
            state
                .runtime_actor(&actor_id)
                .and_then(|actor| actor.actor.current_hp),
            Some(0)
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken {
                origin_id,
                target_id,
                amount: 1,
                remaining_hp: 0,
            } if origin_id == "shambler_front_gate" && target_id == &actor_id
        )));
        assert!(
            outcome
                .lines
                .iter()
                .any(|line| line.contains("incapacitated"))
        );
    }

    #[test]
    fn incapacitated_controlled_actor_can_wait_but_cannot_continue_actions_or_win() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.hp = 0;
        let turn_before = state.turn_index;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }]
                if reason.contains("incapacitated")
        ));
        assert_eq!(state.turn_index, turn_before);
        assert!(!state.active_objective.completed);
        assert!(outcome.events.iter().all(|event| !matches!(
            event,
            GameEvent::RunWon | GameEvent::ObjectiveCompleted { .. }
        )));

        let proxy_action = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: "living_companion".to_owned(),
                target: "front_gate".to_owned(),
            },
        );
        assert!(matches!(
            proxy_action.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("only wait")
        ));

        let wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert!(
            wait.events
                .iter()
                .any(|event| matches!(event, GameEvent::Waited { .. }))
        );
        assert!(state.turn_index > turn_before);
    }

    #[test]
    fn torch_reveals_connected_locations_and_then_stabilizes() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        assert_eq!(state.known_locations.len(), 1);

        let first_use = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "torch".to_owned(),
            },
        );

        assert!(state.known_locations.contains("kitchen"));
        assert!(state.known_locations.contains("garage"));
        assert!(state.known_locations.contains("back_garden"));
        assert!(matches!(
            first_use.events.first(),
            Some(GameEvent::ItemUsed {
                item_id,
                effect: ItemUseEffect::RevealedLocations { count: 3 },
                ..
            }) if item_id == "torch"
        ));
        assert!(first_use.lines.iter().any(|line| {
            line == "You sweep the torch across the exits and get a better read on the nearby routes."
        }));

        let second_use = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "torch".to_owned(),
            },
        );

        assert!(matches!(
            second_use.events.first(),
            Some(GameEvent::ItemUsed {
                item_id,
                effect: ItemUseEffect::RevealedLocations { count: 0 },
                ..
            }) if item_id == "torch"
        ));
        assert_eq!(
            second_use.lines.first().map(String::as_str),
            Some("The torch does not reveal anything new from here.")
        );
    }

    #[test]
    fn locked_garage_requires_house_keys_and_unlocks_cleanly() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let blocked_move = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "front_verandah");
        assert!(state.locked_locations.contains("garage"));
        assert_eq!(
            blocked_move.lines.first().map(String::as_str),
            Some("The garage door is locked. You need the house keys.")
        );
        let blocked_back_garden = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        assert_eq!(
            blocked_back_garden.lines.first().map(String::as_str),
            Some("The back gate is chained shut. You need the house keys.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );

        let wrong_context = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "house_keys".to_owned(),
            },
        );
        assert_eq!(
            wrong_context.lines.first().map(String::as_str),
            Some("That unlock item does not help here.")
        );
        assert!(state.locked_locations.contains("garage"));

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );

        let ambiguous_unlock = apply_action(
            &mut state,
            &bundle,
            GameAction::Use {
                item_name: "house_keys".to_owned(),
            },
        );
        assert_eq!(
            ambiguous_unlock.lines.first().map(String::as_str),
            Some(
                "More than one gate matches this item here: Garage, Back Garden. Use unlock <location>."
            )
        );
        assert!(state.locked_locations.contains("garage"));
        assert!(state.locked_locations.contains("back_garden"));

        let unlock_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        assert!(!state.locked_locations.contains("garage"));
        assert!(unlock_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationUnlocked {
                location_id,
                item_id,
                ..
            } if location_id == "garage" && item_id == "house_keys"
        )));
        assert_eq!(
            unlock_outcome.lines.first().map(String::as_str),
            Some("You unlock Garage with house keys.")
        );

        let moved = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "garage");
        assert!(matches!(
            moved.events.first(),
            Some(GameEvent::Moved { to_location_id, .. }) if to_location_id == "garage"
        ));
    }

    #[test]
    fn unlock_command_alias_unlocks_reachable_gate() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );

        let unlock_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );

        assert!(!state.locked_locations.contains("garage"));
        assert!(unlock_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationUnlocked { location_id, .. } if location_id == "garage"
        )));
    }

    #[test]
    fn explicit_unlock_command_can_target_second_gate() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );

        let unlock_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );

        assert!(!state.locked_locations.contains("back_garden"));
        assert!(state.locked_locations.contains("garage"));
        assert!(unlock_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationUnlocked { location_id, .. } if location_id == "back_garden"
        )));
    }

    #[test]
    fn inspect_location_surfaces_gate_state_and_item_unlock_targets() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let garage_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "garage".to_owned(),
            },
        );
        assert!(
            garage_inspect
                .lines
                .iter()
                .any(|line| line.contains("Gate state: Locked"))
        );
        assert!(
            garage_inspect
                .lines
                .iter()
                .any(|line| line.contains("unlock item: house_keys"))
        );
        let back_garden_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "back_garden".to_owned(),
            },
        );
        assert!(
            back_garden_inspect
                .lines
                .iter()
                .any(|line| line.contains("Gate state: Locked"))
        );
        let front_verandah_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "front_verandah".to_owned(),
            },
        );
        assert!(
            front_verandah_inspect
                .lines
                .iter()
                .any(|line| { line.contains("Barricade state: Unbarricaded") })
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        let key_inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "house_keys".to_owned(),
            },
        );
        assert!(
            key_inspect
                .lines
                .iter()
                .any(|line| line.contains("Can unlock: Garage, Back Garden."))
        );
    }

    #[test]
    fn inspect_enemy_surfaces_live_threat_state_and_route_read() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "back_garden".to_owned();
        state.locked_locations.remove("back_garden");
        state.known_locations.insert("back_garden".to_owned());
        state.visited_locations.insert("back_garden".to_owned());

        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "crawler_in_weeds".to_owned(),
            },
        );

        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Threat state: active | HP remaining: 3")
        );
        assert!(inspect.lines.iter().any(|line| line == "Present here: yes"));
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Senses: hearing yes | sight yes")
        );
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| { line.contains("this is the flank tax") })
        );
    }

    #[test]
    fn inspect_boss_surfaces_wounded_phase_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        state.locked_locations.remove("garage");
        state.known_locations.insert("garage".to_owned());
        state.visited_locations.insert("garage".to_owned());
        state.boss_hp.insert("brute_in_garage".to_owned(), 4);

        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "brute_in_garage".to_owned(),
            },
        );

        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Threat state: active | HP remaining: 4")
        );
        assert!(inspect.lines.iter().any(|line| line == "Present here: yes"));
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line == "Senses: hearing yes | sight yes")
        );
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| { line.contains("Final phase: wounded and swinging harder") })
        );
    }

    #[test]
    fn look_surfaces_context_for_noisy_front_threshold() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.lines.iter().any(|line| {
            line.contains(
                "every loose sound on the property seems to funnel back toward the front step",
            )
        }));
    }

    #[test]
    fn look_surfaces_context_for_cleared_front_threshold() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.enemies_alive.remove("shambler_front_gate");

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.lines.iter().any(|line| {
            line.contains(
                "the front threshold is still a mess, but at least it belongs to you again",
            )
        }));
    }

    #[test]
    fn crawler_retaliation_surfaces_lane_specific_combat_text() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "back_garden".to_owned();
        state.locked_locations.remove("back_garden");
        state.known_locations.insert("back_garden".to_owned());
        state.visited_locations.insert("back_garden".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(
            outcome
                .lines
                .iter()
                .any(|line| { line.contains("It comes in low and hateful") })
        );
    }

    #[test]
    fn look_surfaces_context_for_secured_back_garden() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.lines.iter().any(|line| {
            line.contains("this becomes a rare place to recover 2 HP and regroup")
        }));
    }

    #[test]
    fn waiting_at_front_verandah_hurts_until_it_is_barricaded() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let first_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 9);
        assert_eq!(state.noise_level, 0);
        assert!(first_wait.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken {
                amount: 1,
                remaining_hp: 9,
                ..
            }
        )));

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );

        let wrong_place = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );
        assert_eq!(
            wrong_place.lines.first().map(String::as_str),
            Some("You need to be there to barricade it.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        let barricade = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );

        assert!(state.barricaded_locations.contains("front_verandah"));
        assert!(barricade.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationBarricaded {
                location_id,
                item_id,
                ..
            } if location_id == "front_verandah" && item_id == "barricade_kit"
        )));

        let hp_before_safe_wait = state.hp;
        let safe_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, hp_before_safe_wait);
        assert!(safe_wait.lines.iter().any(|line| {
            line.contains("The barricade takes the edge off the front gate pressure")
        }));
        assert!(
            !safe_wait
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { .. }))
        );
    }

    #[test]
    fn front_verandah_barricade_blocks_shambler_retaliation() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );

        let hp_before_attack = state.hp;
        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.hp, hp_before_attack);
        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                target_id,
                target_kind: EncounterKind::Enemy,
                defeated: false,
                ..
            } if target_id == "shambler_front_gate"
        )));
        assert!(
            !attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { .. }))
        );
        assert!(attack.lines.iter().any(|line| {
            line.contains("The barricade keeps the threat at splinter-spitting distance")
        }));
    }

    #[test]
    fn front_verandah_barricade_adds_attack_bonus_against_the_threshold_threat() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(matches!(
            attack.events.first(),
            Some(GameEvent::AttackResolved {
                target_kind: EncounterKind::Enemy,
                damage: 2,
                defeated: false,
                ..
            })
        ));
        assert!(
            attack
                .lines
                .iter()
                .any(|line| { line.contains("Attack bonus: +1") })
        );
    }

    #[test]
    fn waiting_in_back_garden_hurts_until_it_is_barricaded() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );

        let first_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert!(
            first_wait
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 1, .. }))
        );
        let hp_after_wait = state.hp;

        let barricade = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );
        assert!(state.barricaded_locations.contains("back_garden"));
        assert!(barricade.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationBarricaded {
                location_id,
                item_id,
                ..
            } if location_id == "back_garden" && item_id == "barricade_kit"
        )));
        assert_eq!(state.hp, (hp_after_wait + 2).min(state.max_hp));
        let restored = state.hp - hp_after_wait;
        assert!(barricade.events.iter().any(|event| matches!(
            event,
            GameEvent::HealthRestored {
                origin_id,
                target_id,
                amount,
                ..
            } if origin_id == "host barricade system" && target_id == "player" && *amount == restored
        )));
        assert!(
            barricade
                .lines
                .iter()
                .any(|line| { line.contains("HP rises from") })
        );

        let hp_before_safe_wait = state.hp;
        let safe_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, hp_before_safe_wait);
        assert!(safe_wait.lines.iter().any(|line| {
            line.contains("The back barricade keeps the weeds from becoming a bite problem")
        }));
        assert!(
            !safe_wait
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { .. }))
        );
    }

    #[test]
    fn loud_actions_raise_noise_and_successful_wait_lowers_it() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert_eq!(state.noise_level, 1);
        assert!(
            attack
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        let unlock = apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        assert_eq!(state.noise_level, 1);
        assert!(
            unlock
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        let barricade = apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );
        assert_eq!(state.noise_level, 1);
        assert!(
            barricade
                .lines
                .iter()
                .any(|line| line == "Noise rises to Stirred.")
        );

        let calm_wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.noise_level, 0);
        assert!(
            calm_wait
                .lines
                .iter()
                .any(|line| line == "Noise settles to Quiet.")
        );
    }

    #[test]
    fn successful_non_noisy_actions_reduce_noise_over_time() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let look = apply_action(&mut state, &bundle, GameAction::Look);

        assert_eq!(state.noise_level, 1);
        assert!(
            look.lines
                .iter()
                .any(|line| line == "Noise settles to Stirred.")
        );

        let move_outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert_eq!(state.noise_level, 0);
        assert!(
            move_outcome
                .lines
                .iter()
                .any(|line| line == "Noise settles to Quiet.")
        );
    }

    #[test]
    fn rejected_non_noisy_actions_do_not_reduce_noise() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let blocked = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        assert_eq!(state.noise_level, 2);
        assert!(
            blocked
                .events
                .iter()
                .any(|event| { matches!(event, GameEvent::MovementBlocked { .. }) })
        );
        assert!(
            !blocked
                .lines
                .iter()
                .any(|line| line.starts_with("Noise settles"))
        );
    }

    #[test]
    fn max_noise_spawns_enemy_instance_in_yard_location() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        // The live crawler fixture already occupies the yard. Remove that one instance to test
        // the pool's materialization path rather than asking the pool to duplicate a living
        // template in the same run.
        state.enemies_alive.remove("crawler_in_weeds");
        state.enemy_hp.remove("crawler_in_weeds");
        state.enemy_instances.remove("crawler_in_weeds");
        for entries in state.location_enemies.values_mut() {
            entries.retain(|entry| entry != "crawler_in_weeds");
        }
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        let Some(GameEvent::ActorSpawnedFromNoise {
            origin_id: enemy_id,
            template_id,
            location_id,
        }) = outcome
            .events
            .iter()
            .find(|event| matches!(event, GameEvent::ActorSpawnedFromNoise { .. }))
        else {
            panic!("expected max-noise spawn event");
        };
        let spawn_location = bundle
            .locations
            .iter()
            .find(|location| location.id == *location_id)
            .expect("spawned location should resolve");
        let spawned_template = bundle
            .enemies
            .iter()
            .find(|enemy| enemy.id == *template_id)
            .expect("spawned enemy template should resolve");

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 1);
        assert!(enemy_id.starts_with("noise_spawn_1_"));
        assert!(spawn_location.tags.iter().any(|tag| tag == "outdoor"));
        assert!(state.enemies_alive.contains(enemy_id));
        assert_eq!(state.enemy_hp.get(enemy_id), Some(&spawned_template.hp));
        assert!(
            state
                .location_enemies
                .get(location_id)
                .is_some_and(|entries| entries.contains(enemy_id))
        );
        assert!(outcome.lines.iter().any(|line| {
            line.contains("Noise peaks at Swarming") && line.contains(&spawn_location.name)
        }));
        assert!(!outcome.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::ActorMoved { origin_id: moved_id, .. }
                    | GameEvent::ActorWaited {
                        origin_id: moved_id,
                        ..
                    } if moved_id == enemy_id
            )
        }));

        let spawned_enemy_id = enemy_id.clone();
        let spawned_template_id = template_id.clone();
        let spawned_location_id = location_id.clone();
        state.current_location_id = spawned_location_id;
        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: spawned_template_id,
            },
        );
        assert!(matches!(
            inspect.events.first(),
            Some(GameEvent::Inspected { target, .. }) if target == &spawned_enemy_id
        ));
    }

    #[test]
    fn max_noise_does_not_duplicate_an_active_enemy_template() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 0);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ActorSpawnedFromNoise { .. }))
        );
    }

    #[test]
    fn noise_spawn_only_fires_when_noise_crosses_into_maximum() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 3;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 0);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| { matches!(event, GameEvent::ActorSpawnedFromNoise { .. }) })
        );
    }

    #[test]
    fn max_noise_does_not_spawn_enemy_when_pool_cannot_hear() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        for enemy in &mut bundle.enemies {
            enemy.can_hear = false;
        }
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(state.noise_level, 3);
        assert_eq!(state.noise_spawn_count, 0);
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| { matches!(event, GameEvent::ActorSpawnedFromNoise { .. }) })
        );
    }

    #[test]
    fn tagged_npc_can_join_the_noise_pool_and_use_shared_tracking() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        for enemy in &mut bundle.enemies {
            enemy.can_hear = false;
        }
        let npc = bundle
            .npcs
            .iter_mut()
            .find(|npc| npc.id == "kitchen_neighbour")
            .expect("fixture NPC");
        npc.disposition = "hostile".to_owned();
        npc.actor.hp = Some(2);
        npc.actor.damage = Some(1);
        npc.actor.can_hear = true;
        npc.actor.can_see = true;
        npc.actor.can_move = true;
        npc.actor.combat = true;
        npc.capabilities = vec![
            "health".to_owned(),
            "combat".to_owned(),
            "hearing".to_owned(),
            "sight".to_owned(),
            "noise_spawn".to_owned(),
            "noise_track".to_owned(),
            "sight_track".to_owned(),
            "movement".to_owned(),
        ];

        let mut state = generate_new_run(&bundle).state;
        // Its authored placement is live at run start. Remove it so this test verifies that the
        // same NPC template can enter through the shared noise-pool path once no copy is active.
        for entries in state.location_npcs.values_mut() {
            entries.retain(|entry| entry.template_id != "kitchen_neighbour");
        }
        state.noise_level = 2;
        let spawned = apply_action(&mut state, &bundle, GameAction::Attack);
        let (npc_instance_id, npc_location_id) = spawned
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::ActorSpawnedFromNoise {
                    origin_id: enemy_id,
                    template_id,
                    location_id,
                } if template_id == "kitchen_neighbour" => {
                    Some((enemy_id.clone(), location_id.clone()))
                }
                _ => None,
            })
            .expect("tagged NPC should be selected from the shared noise pool");
        let spawned_npc = state.location_npcs[&npc_location_id]
            .iter()
            .find(|npc| npc.instance_id == npc_instance_id)
            .expect("noise pool NPC is materialized in the normal NPC bucket");
        assert!(spawned_npc.actor.can_hear);
        assert!(spawned_npc.actor.can_see);
        assert!(spawned_npc.actor.can_move);
        assert_eq!(
            state.actor_noise_targets.get(&npc_instance_id),
            Some(&state.current_location_id)
        );

        let next_turn = apply_action(&mut state, &bundle, GameAction::Wait);
        assert!(next_turn.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::ActorMoved { origin_id, .. }
                    | GameEvent::ActorWaited { origin_id, .. }
                    if origin_id == &npc_instance_id
            )
        }));
    }

    #[test]
    fn tagged_boss_can_join_the_shared_noise_pool_without_a_second_spawner() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        for enemy in &mut bundle.enemies {
            enemy.can_hear = false;
        }
        for npc in &mut bundle.npcs {
            npc.actor.can_hear = false;
        }
        let boss = bundle.bosses.first_mut().expect("fixture boss");
        boss.can_hear = true;
        boss.can_see = true;
        boss.capabilities = vec![
            "hearing".to_owned(),
            "sight".to_owned(),
            "noise_spawn".to_owned(),
            "noise_track".to_owned(),
            "sight_track".to_owned(),
            "movement".to_owned(),
        ];
        let boss_template_id = boss.id.clone();

        let mut state = generate_new_run(&bundle).state;
        // Do not create a second living Garage Brute just to exercise the common pool path.
        state.bosses_alive.remove(&boss_template_id);
        state.boss_hp.remove(&boss_template_id);
        state.boss_instances.remove(&boss_template_id);
        for entries in state.location_bosses.values_mut() {
            entries.retain(|entry| entry != &boss_template_id);
        }
        state.noise_level = 2;
        let spawned = apply_action(&mut state, &bundle, GameAction::Attack);
        let boss_instance_id = spawned
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::ActorSpawnedFromNoise {
                    origin_id: enemy_id,
                    template_id,
                    ..
                } if template_id == &boss_template_id => Some(enemy_id.clone()),
                _ => None,
            })
            .expect("tagged boss should use the same noise pool");
        assert!(state.bosses_alive.contains(&boss_instance_id));
        assert!(state.boss_instances.contains_key(&boss_instance_id));

        let next_turn = apply_action(&mut state, &bundle, GameAction::Wait);
        assert!(next_turn.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::ActorMoved { origin_id, .. }
                    | GameEvent::ActorWaited { origin_id, .. }
                    if origin_id == &boss_instance_id
            )
        }));
    }

    #[test]
    fn existing_spawned_enemy_moves_one_legal_tile_toward_noise_source() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "laundry".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(
            !state
                .location_enemies
                .get("back_garden")
                .is_some_and(|entries| entries.contains(&enemy_id))
        );
        assert!(
            state
                .location_enemies
                .get("front_verandah")
                .is_some_and(|entries| entries.contains(&enemy_id))
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                from_location_id,
                to_location_id,
                target_location_id,
            } if moved_id == &enemy_id
                && from_location_id == "back_garden"
                && to_location_id == "front_verandah"
                && target_location_id == "front_verandah"
        )));
    }

    #[test]
    fn noisy_actions_shift_existing_spawned_enemy_attractor_to_latest_source() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "kitchen".to_owned();
        state.noise_level = 3;
        state.noise_spawn_count = 1;
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        state.actor_searching.insert(enemy_id.clone());

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert_eq!(
            state.actor_noise_targets.get(&enemy_id).map(String::as_str),
            Some("kitchen")
        );
        assert!(!state.actor_searching.contains(&enemy_id));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::NoiseAttractorShifted {
                location_id,
                actor_ids,
            } if location_id == "kitchen" && actor_ids == &vec![enemy_id.clone()]
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                from_location_id,
                to_location_id,
                target_location_id,
            } if moved_id == &enemy_id
                && from_location_id == "back_garden"
                && to_location_id == "front_verandah"
                && target_location_id == "kitchen"
        )));
    }

    #[test]
    fn spawned_enemy_searches_after_reaching_noise_source() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "laundry".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("front_verandah".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        state
            .actor_origins
            .insert(enemy_id.clone(), "kitchen".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(state.actor_searching.contains(&enemy_id));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorWaited {
                origin_id: waited_id,
                reason,
                ..
            } if waited_id == &enemy_id && reason.contains("searching")
        ) || matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                from_location_id,
                ..
            } if moved_id == &enemy_id && from_location_id == "front_verandah"
        )));
    }

    #[test]
    fn spawned_enemy_acquires_player_by_sight_and_prioritizes_visual_chase() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut acquired = false;

        for turn_index in 0..40 {
            let mut state = generate_new_run(&bundle).state;
            state.current_location_id = "front_verandah".to_owned();
            state.turn_index = turn_index;
            state.enemies_alive.insert(enemy_id.clone());
            state.enemy_hp.insert(enemy_id.clone(), 3);
            state
                .location_enemies
                .entry("kitchen".to_owned())
                .or_default()
                .push(enemy_id.clone());
            state
                .actor_noise_targets
                .insert(enemy_id.clone(), "laundry".to_owned());
            state
                .actor_origins
                .insert(enemy_id.clone(), "back_garden".to_owned());

            let outcome = apply_action(&mut state, &bundle, GameAction::Look);
            if !outcome.events.iter().any(|event| {
                matches!(event, GameEvent::SightAttractorAcquired { origin_id: sighting_id, .. } if sighting_id == &enemy_id)
            }) {
                continue;
            }

            assert_eq!(
                state.actor_sight_targets.get(&enemy_id).map(String::as_str),
                Some("front_verandah")
            );
            assert_eq!(
                state
                    .actor_sight_subjects
                    .get(&enemy_id)
                    .map(String::as_str),
                Some("player")
            );
            assert!(outcome.events.iter().any(|event| matches!(
                event,
                GameEvent::SightAttractorAcquired {
                    origin_id: sighting_id,
                    subject_id,
                    location_id,
                } if sighting_id == &enemy_id
                    && subject_id == "player"
                    && location_id == "front_verandah"
            )));
            assert!(outcome.events.iter().any(|event| matches!(
                event,
                GameEvent::ActorMoved {
                    origin_id: moved_id,
                    target_location_id,
                    ..
                } if moved_id == &enemy_id && target_location_id == "front_verandah"
            ) || matches!(
                event,
                GameEvent::ActorWaited {
                    origin_id: waited_id,
                    reason,
                    ..
                } if waited_id == &enemy_id && reason.contains("chasing player by sight")
            )));
            acquired = true;
            break;
        }

        assert!(
            acquired,
            "expected at least one deterministic sight acquisition in searched states"
        );
    }

    #[test]
    fn rules_can_disable_sight_acquisition_chance() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!state.actor_sight_targets.contains_key(&enemy_id));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorMissed {
                origin_id: missed_id,
                detect_chance_percent: 0,
                ..
            } if missed_id == &enemy_id
        )));
    }

    #[test]
    fn rules_can_disable_sight_chase_delay() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        bundle.rules.sight_chase_delay_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                target_location_id,
                ..
            } if moved_id == &enemy_id && target_location_id == "front_verandah"
        )));
        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorWaited {
                origin_id: waited_id,
                reason,
                ..
            } if waited_id == &enemy_id && reason.contains("chasing player by sight")
        )));
    }

    #[test]
    fn player_sight_overrides_active_noise_attractor() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        bundle.rules.sight_chase_delay_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "laundry".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired {
                origin_id: sighting_id,
                subject_id,
                location_id,
            } if sighting_id == &enemy_id
                && subject_id == "player"
                && location_id == "front_verandah"
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                to_location_id,
                target_location_id,
                ..
            } if moved_id == &enemy_id
                && to_location_id == "front_verandah"
                && target_location_id == "front_verandah"
        )));
    }

    #[test]
    fn sight_tracking_targets_the_actual_controlled_actor_not_the_legacy_player_id() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        bundle.rules.sight_chase_delay_chance_percent = 0;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let controller_id = "sight_controller_fixture".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.location_npcs.insert(
            "front_verandah".to_owned(),
            vec![NpcInstance {
                instance_id: controller_id.clone(),
                template_id: "sight_controller_fixture".to_owned(),
                location_id: "front_verandah".to_owned(),
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Sight controller".to_owned(),
                description: "The active controlled actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState::configured(Some(5), None, false, false, false, false),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.player_entity_id = controller_id.clone();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "laundry".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert_eq!(
            state
                .actor_sight_subjects
                .get(&enemy_id)
                .map(String::as_str),
            Some(controller_id.as_str())
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired { origin_id, subject_id, .. }
                if origin_id == &enemy_id && subject_id == &controller_id
        )));
    }

    #[test]
    fn non_player_sight_does_not_override_active_noise_attractor() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.sight_acquire_chance_percent = 100;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "garage".to_owned();
        state.location_enemies.clear();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .location_enemies
            .entry("laundry".to_owned())
            .or_default()
            .push("crawler_in_weeds".to_owned());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired {
                origin_id: sighting_id,
                subject_id,
                ..
            } if sighting_id == &enemy_id && subject_id == "crawler_in_weeds"
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved {
                origin_id: moved_id,
                target_location_id,
                ..
            } if moved_id == &enemy_id && target_location_id == "front_verandah"
        )));
    }

    #[test]
    fn spawned_enemy_cannot_acquire_sight_when_template_cannot_see() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let shambler = bundle
            .enemies
            .iter_mut()
            .find(|enemy| enemy.id == "shambler_front_gate")
            .expect("expected shambler template");
        shambler.can_see = false;
        let mut state = generate_new_run(&bundle).state;
        state.current_location_id = "front_verandah".to_owned();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_origins
            .insert(enemy_id.clone(), "back_garden".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!state.actor_sight_targets.contains_key(&enemy_id));
        assert!(!outcome.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::SightAttractorAcquired { origin_id: sighting_id, .. }
                    | GameEvent::SightAttractorMissed { origin_id: sighting_id, .. }
                    if sighting_id == &enemy_id
            )
        }));
    }

    #[test]
    fn spawned_enemy_sight_check_can_fail_before_acquisition() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        let mut miss_outcome = None;

        for turn_index in 0..40 {
            let mut state = generate_new_run(&bundle).state;
            state.current_location_id = "front_verandah".to_owned();
            state.turn_index = turn_index;
            state.enemies_alive.insert(enemy_id.clone());
            state.enemy_hp.insert(enemy_id.clone(), 3);
            state
                .location_enemies
                .entry("kitchen".to_owned())
                .or_default()
                .push(enemy_id.clone());
            state
                .actor_origins
                .insert(enemy_id.clone(), "back_garden".to_owned());

            let outcome = apply_action(&mut state, &bundle, GameAction::Look);
            if outcome.events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::SightAttractorMissed {
                        origin_id: missed_id,
                        subject_id,
                        location_id,
                        detect_chance_percent,
                        ..
                    } if missed_id == &enemy_id
                        && subject_id == "player"
                        && location_id == "front_verandah"
                        && *detect_chance_percent == bundle.rules.sight_acquire_chance_percent
                )
            }) {
                assert!(!state.actor_sight_targets.contains_key(&enemy_id));
                miss_outcome = Some(outcome);
                break;
            }
        }

        assert!(
            miss_outcome.is_some(),
            "expected at least one deterministic sight miss in searched states"
        );
    }

    #[test]
    fn delayed_sight_chase_can_be_shaken_into_search() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "garage".to_owned();
        state.location_enemies.clear();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .actor_noise_targets
            .insert(enemy_id.clone(), "laundry".to_owned());
        state
            .actor_origins
            .insert(enemy_id.clone(), "back_garden".to_owned());
        state
            .actor_sight_targets
            .insert(enemy_id.clone(), "front_verandah".to_owned());
        state
            .actor_sight_subjects
            .insert(enemy_id.clone(), "player".to_owned());
        state.actor_sight_delays.insert(enemy_id.clone(), 1);

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(state.actor_searching.contains(&enemy_id));
        assert!(!state.actor_sight_targets.contains_key(&enemy_id));
        assert!(!state.actor_sight_subjects.contains_key(&enemy_id));
        assert!(!state.actor_sight_delays.contains_key(&enemy_id));
        assert!(outcome.lines.iter().any(|line| line.contains("searching")));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorLost {
                origin_id: lost_id,
                subject_id,
            } if lost_id == &enemy_id && subject_id == "player"
        )));
    }

    #[test]
    fn spawned_enemy_can_acquire_other_enemy_by_sight() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.current_location_id = "garage".to_owned();
        state.location_enemies.clear();
        state.enemies_alive.insert(enemy_id.clone());
        state.enemy_hp.insert(enemy_id.clone(), 3);
        state
            .location_enemies
            .entry("kitchen".to_owned())
            .or_default()
            .push(enemy_id.clone());
        state
            .location_enemies
            .entry("laundry".to_owned())
            .or_default()
            .push("crawler_in_weeds".to_owned());

        let outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert_eq!(
            state.actor_sight_targets.get(&enemy_id).map(String::as_str),
            Some("laundry")
        );
        assert_eq!(
            state
                .actor_sight_subjects
                .get(&enemy_id)
                .map(String::as_str),
            Some("crawler_in_weeds")
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SightAttractorAcquired {
                origin_id: sighting_id,
                subject_id,
                location_id,
            } if sighting_id == &enemy_id
                && subject_id == "crawler_in_weeds"
                && location_id == "laundry"
        )));
    }

    #[test]
    fn searching_spawned_enemy_can_choose_to_return_to_origin() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let enemy_id = "noise_spawn_1_shambler_front_gate".to_owned();
        state.noise_spawn_count = 1;
        state
            .actor_origins
            .insert(enemy_id.clone(), "kitchen".to_owned());

        let mut return_step = None;
        for turn_index in 0..20 {
            state.turn_index = turn_index;
            if let SpawnedEnemyStep::Move {
                to_location_id,
                step_target_location_id: Some(step_target_location_id),
            } = spawned_enemy_next_step(
                &state,
                &bundle,
                &enemy_id,
                "front_verandah",
                "front_verandah",
                true,
            ) {
                return_step = Some((to_location_id, step_target_location_id));
                break;
            }
        }

        assert_eq!(
            return_step,
            Some(("kitchen".to_owned(), "kitchen".to_owned()))
        );
    }

    #[test]
    fn spawned_enemy_attacks_barricade_hazard_and_can_fail_to_break_it() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let barricade_blocked_id = "noise_spawn_1_crawler_in_weeds".to_owned();
        state.enemies_alive.insert(barricade_blocked_id.clone());
        state.enemy_hp.insert(barricade_blocked_id.clone(), 2);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(barricade_blocked_id.clone());
        state
            .actor_noise_targets
            .insert(barricade_blocked_id.clone(), "front_verandah".to_owned());
        state
            .barricaded_locations
            .insert("front_verandah".to_owned());
        state.status_effects.push(StatusEffectInstance {
            instance_id: "hazard_strike_mend".to_owned(),
            template_id: "hazard_strike_mend".to_owned(),
            owner: StatusOwner::Entity,
            owner_id: Some(barricade_blocked_id.clone()),
            trigger: EffectTrigger::Combat,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 1,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        let barricade_outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(
            state
                .location_enemies
                .get("back_garden")
                .is_some_and(|entries| entries.contains(&barricade_blocked_id))
        );
        assert!(state.barricaded_locations.contains("front_verandah"));
        assert!(barricade_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorAttackedHazard {
                origin_id,
                hazard_kind: MovementHazardKind::Barricade,
                location_id,
                broken: false,
                ..
            } if origin_id == &barricade_blocked_id
                && location_id == "front_verandah"
        )));
        assert!(matches!(
            state
                .status_effects
                .iter()
                .find(|effect| effect.instance_id == "hazard_strike_mend")
                .map(|effect| &effect.lifecycle),
            Some(InstanceLifecycle::Consumed)
        ));
        assert!(barricade_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "hazard_strike_mend"
        )));
    }

    #[test]
    fn rules_can_force_spawned_hazard_breaks() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        bundle.rules.spawned_hazard_break_chance_percent = 100;
        let mut state = generate_new_run(&bundle).state;
        let barricade_blocked_id = "noise_spawn_1_crawler_in_weeds".to_owned();
        state.enemies_alive.insert(barricade_blocked_id.clone());
        state.enemy_hp.insert(barricade_blocked_id.clone(), 2);
        state
            .location_enemies
            .entry("back_garden".to_owned())
            .or_default()
            .push(barricade_blocked_id.clone());
        state
            .actor_noise_targets
            .insert(barricade_blocked_id.clone(), "front_verandah".to_owned());
        state
            .barricaded_locations
            .insert("front_verandah".to_owned());

        let barricade_outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(!state.barricaded_locations.contains("front_verandah"));
        assert!(barricade_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorAttackedHazard {
                origin_id,
                hazard_kind: MovementHazardKind::Barricade,
                location_id,
                break_chance_percent: 100,
                broken: true,
                ..
            } if origin_id == &barricade_blocked_id
                && location_id == "front_verandah"
        )));
    }

    #[test]
    fn spawned_enemy_can_break_locked_gate_into_broken_open_state() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        let locked_blocked_id = "noise_spawn_2_shambler_front_gate".to_owned();
        state.current_location_id = "laundry".to_owned();
        state.noise_spawn_count = 1;
        state.enemies_alive.insert(locked_blocked_id.clone());
        state.enemy_hp.insert(locked_blocked_id.clone(), 3);
        state
            .location_enemies
            .entry("front_verandah".to_owned())
            .or_default()
            .push(locked_blocked_id.clone());
        state
            .actor_noise_targets
            .insert(locked_blocked_id.clone(), "garage".to_owned());

        let locked_outcome = apply_action(&mut state, &bundle, GameAction::Look);

        assert!(
            state
                .location_enemies
                .get("front_verandah")
                .is_some_and(|entries| entries.contains(&locked_blocked_id))
        );
        assert!(locked_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorAttackedHazard {
                origin_id,
                hazard_kind: MovementHazardKind::LockedGate,
                location_id,
                broken: true,
                ..
            } if origin_id == &locked_blocked_id
                && location_id == "garage"
        )));
        assert!(!state.locked_locations.contains("garage"));
        assert!(state.broken_locked_locations.contains("garage"));

        state.current_location_id = "front_verandah".to_owned();
        let inspect = apply_action(
            &mut state,
            &bundle,
            GameAction::Inspect {
                target: "garage".to_owned(),
            },
        );
        assert!(
            inspect
                .lines
                .iter()
                .any(|line| line.contains("Gate state: Broken"))
        );

        let moved = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "garage");
        assert!(moved.events.iter().any(|event| {
            matches!(event, GameEvent::Moved { to_location_id, .. } if to_location_id == "garage")
        }));
    }

    #[test]
    fn exposed_pressure_and_retaliation_scale_up_at_high_noise() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.noise_level = 2;

        let wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 8);
        assert_eq!(state.noise_level, 1);
        assert!(wait.events.iter().any(|event| matches!(
            event,
            GameEvent::DamageTaken {
                amount: 2,
                remaining_hp: 8,
                ..
            }
        )));

        state.noise_level = 2;
        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 3, .. }))
        );
    }

    #[test]
    fn boss_combat_completes_objective_and_surfaces_win() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "cricket_bat".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        assert!(!state.active_objective.completed);
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        let first = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(matches!(
            first.events.first(),
            Some(GameEvent::AttackResolved {
                target_kind: EncounterKind::Boss,
                damage: 3,
                defeated: false,
                ..
            })
        ));
        assert_eq!(state.hp, 7);
        assert!(state.bosses_alive.contains("brute_in_garage"));

        apply_action(&mut state, &bundle, GameAction::Attack);
        let final_outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(!state.bosses_alive.contains("brute_in_garage"));
        assert!(state.bosses_defeated.contains("brute_in_garage"));
        assert!(state.active_objective.completed);
        assert!(final_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                target_kind: EncounterKind::Boss,
                defeated: true,
                ..
            }
        )));
        assert!(
            final_outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
        assert!(
            final_outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::RunWon))
        );
        assert!(final_outcome.lines.iter().any(|line| {
            line == "Objective progress: Defeat boss 'brute_in_garage' is now complete."
        }));
        assert!(final_outcome.lines.iter().any(|line| line == "You win."));
    }

    #[test]
    fn garage_brute_wounded_phase_hits_harder_and_surfaces_phase_text() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        state.boss_hp.insert("brute_in_garage".to_owned(), 5);
        let hp_before = state.hp;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(attack.lines.iter().any(|line| {
            line.contains("becomes more dangerous once it realizes it should already be dead")
        }));
        assert!(attack.lines.iter().any(|line| {
            line.contains("Final-phase pressure: the brute is hitting harder now")
        }));
        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 4, .. }))
        );
        assert_eq!(state.hp, hp_before - 4);
    }

    #[test]
    fn secured_siege_lanes_reduce_garage_brute_retaliation() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Equip {
                item_name: "cricket_bat".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "barricade_kit".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "back_garden".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Barricade {
                target: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        state.boss_hp.insert("brute_in_garage".to_owned(), 5);
        let hp_before = state.hp;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::DamageTaken { amount: 3, .. }))
        );
        assert_eq!(state.hp, hp_before - 3);
        assert!(
            attack
                .lines
                .iter()
                .any(|line| { line.contains("Retaliation reduced by 1") })
        );
        assert!(
            attack
                .lines
                .iter()
                .any(|line| { line.contains("Final-phase pressure") })
        );
    }

    #[test]
    fn item_only_objective_completes_when_required_item_is_taken() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.target_boss_id = None;
        state.active_objective.required_item_id = Some("house_keys".to_owned());
        state.active_objective.required_location_id = None;
        state.active_objective.completed = false;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );

        assert!(state.active_objective.completed);
        assert!(
            outcome.lines.iter().any(|line| {
                line == "Objective progress: Hold item 'house_keys' is now complete."
            })
        );
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
        assert!(outcome.lines.iter().any(|line| line == "You win."));
    }

    #[test]
    fn location_only_objective_completes_when_required_location_is_reached() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.target_boss_id = None;
        state.active_objective.required_item_id = None;
        state.active_objective.required_location_id = Some("kitchen".to_owned());
        state.active_objective.completed = false;

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert!(state.active_objective.completed);
        assert!(outcome.lines.iter().any(|line| {
            line == "Objective progress: Reach location 'kitchen' is now complete."
        }));
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ObjectiveCompleted { .. }))
        );
    }

    #[test]
    fn combined_objective_requires_location_in_addition_to_other_conditions() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Take {
                item_name: "house_keys".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        apply_action(
            &mut state,
            &bundle,
            GameAction::Unlock {
                target: "garage".to_owned(),
            },
        );

        state.active_objective.target_boss_id = None;
        state.active_objective.required_item_id = Some("house_keys".to_owned());
        state.active_objective.required_location_id = Some("garage".to_owned());
        state.active_objective.completed = false;

        let before_entering = apply_action(&mut state, &bundle, GameAction::Look);
        assert!(!state.active_objective.completed);
        assert!(!before_entering.lines.iter().any(|line| line == "You win."));

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "garage".to_owned(),
            },
        );

        assert!(state.active_objective.completed);
        assert!(outcome.lines.iter().any(|line| {
            line == "Objective progress: Reach location 'garage' is now complete."
        }));
        assert!(outcome.lines.iter().any(|line| line == "You win."));
    }

    #[test]
    fn epilogue_mode_blocks_combat_and_pressure_without_blocking_exploration() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.completed = true;
        state.current_location_id = "garage".to_owned();
        state.locked_locations.remove("garage");
        state.noise_level = 3;
        state.hp = 5;

        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert_eq!(state.hp, 5);
        assert_eq!(state.noise_level, 3);
        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::ActionRejected { reason } if reason.contains("already won")
        )));

        let wait = apply_action(&mut state, &bundle, GameAction::Wait);
        assert_eq!(state.hp, 5);
        assert_eq!(state.noise_level, 3);
        assert!(
            wait.lines
                .iter()
                .any(|line| line.contains("siege clock is no longer spending your HP"))
        );

        let look = apply_action(&mut state, &bundle, GameAction::Look);
        assert!(look.events.iter().any(|event| matches!(
            event,
            GameEvent::LocationLooked { location_id, .. } if location_id == "garage"
        )));
        assert!(
            look.lines
                .iter()
                .any(|line| line.contains("finally gives up being an arena"))
        );
        assert!(
            look.lines
                .iter()
                .any(|line| line.contains("Aftermath hook: Post-credits hook"))
        );

        let move_out = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "front_verandah".to_owned(),
            },
        );
        assert_eq!(state.current_location_id, "front_verandah");
        assert!(move_out.events.iter().any(|event| matches!(
            event,
            GameEvent::Moved {
                to_location_id, ..
            } if to_location_id == "front_verandah"
        )));
    }

    #[test]
    fn rolling_summary_is_bounded_and_event_confirmed() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;

        for _ in 0..(ROLLING_SUMMARY_LIMIT + 8) {
            apply_action(&mut state, &bundle, GameAction::Look);
        }

        assert_eq!(state.rolling_summary.len(), ROLLING_SUMMARY_LIMIT);
        assert!(
            state
                .rolling_summary
                .iter()
                .all(|line| line == "Action [inspection] player -> front_verandah: observed")
        );
    }

    #[test]
    fn inspect_effect_heals_and_expires_through_the_reducer() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state.hp = 5;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "inspect_mend".to_owned(),
            template_id: "inspect_mend".to_owned(),
            owner: StatusOwner::Player,
            owner_id: None,
            trigger: EffectTrigger::Inspect,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 2,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });
        let outcome = apply_action(&mut state, &bundle, GameAction::Look);
        assert_eq!(state.hp, 7);
        assert!(matches!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        ));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "inspect_mend"
        )));
    }

    #[test]
    fn a_combat_whiff_triggers_only_the_actor_owned_failure_status() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state.hp = 5;
        state.enemies_alive.clear();
        state.bosses_alive.clear();
        state.location_npcs.clear();
        state.status_effects.push(StatusEffectInstance {
            instance_id: "failure_second_wind".to_owned(),
            template_id: "failure_second_wind".to_owned(),
            owner: StatusOwner::Player,
            owner_id: None,
            trigger: EffectTrigger::Failure,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 2,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(outcome.events.iter().any(
            |event| matches!(event, GameEvent::AttackWhiff { origin_id } if origin_id == "player")
        ));
        assert_eq!(state.hp, 7);
        assert!(matches!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        ));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "failure_second_wind"
        )));
        assert_eq!(
            state.turn_index, 1,
            "a whiff still spends its ordinary action time"
        );
    }

    #[test]
    fn moving_expires_scene_duration_effects() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "scene_effect".to_owned(),
            template_id: "scene_effect".to_owned(),
            owner: StatusOwner::Scene,
            owner_id: Some("front_verandah".to_owned()),
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::Scene,
            additive_damage: 0,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );
        assert!(matches!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        ));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "scene_effect"
        )));
    }

    #[test]
    fn one_actors_movement_does_not_expire_another_entitys_scene_duration_status() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "npc_scene_effect".to_owned(),
            template_id: "npc_scene_effect".to_owned(),
            owner: StatusOwner::Entity,
            owner_id: Some("other_actor".to_owned()),
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::Scene,
            additive_damage: 0,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert_eq!(state.status_effects[0].lifecycle, InstanceLifecycle::Active);
    }

    #[test]
    fn timing_status_adjusts_cost_and_consumes_only_after_success() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "slow_stride".to_owned(),
            template_id: "slow_stride".to_owned(),
            owner: StatusOwner::Scene,
            owner_id: None,
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 0,
            timing_cost_delta: 4,
            timing_action_kinds: vec![ActionTimingKind::Movement],
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert_eq!(state.turn_index, 5);
        assert_eq!(state.actor_ready_at("player"), 5);
        assert_eq!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "slow_stride"
        )));
    }

    #[test]
    fn movement_ability_triggers_the_same_movement_status_contract() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("fixture starter ability")
            .effect = Some(crate::data::datapacks::AbilityEffect::Move);
        let mut state = generate_new_run(&bundle).state;
        state.hp = 5;
        let grant_id = state.ability_grants[0].instance_id.clone();
        state.status_effects.push(StatusEffectInstance {
            instance_id: "movement_ability_restoration".to_owned(),
            template_id: "movement_ability_restoration".to_owned(),
            owner: StatusOwner::Player,
            owner_id: None,
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 2,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: "kitchen".to_owned(),
            },
        );

        assert_eq!(state.current_location_id, "kitchen");
        assert_eq!(state.hp, 7);
        assert_eq!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id }
                if instance_id == "movement_ability_restoration"
        )));
    }

    #[test]
    fn timing_gate_rejects_without_moving_or_consuming_status() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        let origin = state.current_location_id.clone();
        state.status_effects.push(StatusEffectInstance {
            instance_id: "movement_gate".to_owned(),
            template_id: "movement_gate".to_owned(),
            owner: StatusOwner::Scene,
            owner_id: None,
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::UntilUsed,
            additive_damage: 0,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: vec![ActionTimingKind::Movement],
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { .. }]
        ));
        assert_eq!(state.current_location_id, origin);
        assert_eq!(state.turn_index, 0);
        assert_eq!(state.actor_ready_at("player"), 0);
        assert_eq!(state.status_effects[0].lifecycle, InstanceLifecycle::Active);
    }

    #[test]
    fn rolling_summary_records_epilogue_actions_without_reopening_danger() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut state = generate_new_run(&bundle).state;
        state.active_objective.completed = true;
        state.hp = 5;
        state.noise_level = 3;

        apply_action(&mut state, &bundle, GameAction::Wait);

        assert_eq!(state.hp, 5);
        assert_eq!(state.noise_level, 3);
        assert!(state.rolling_summary.iter().any(|line| {
            line.contains(
                "Action [rejected] host validator -> requested action: The run is already won",
            )
        }));
    }

    #[test]
    fn basic_attack_uses_equipped_template_cost_and_sets_player_readiness() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        for item in &mut bundle.items {
            item.attack_cost = 7;
        }
        let mut state = generate_new_run(&bundle).state;

        let outcome = apply_action(&mut state, &bundle, GameAction::Attack);

        assert!(outcome
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { origin_id, .. } if origin_id == "player")));
        assert_eq!(state.turn_index, 7);
        assert_eq!(state.actor_ready_at("player"), 7);
        assert!(state.actor_is_ready("player"));
    }

    #[test]
    fn ability_action_cost_and_grant_cooldown_are_independent() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let ability = bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter ability");
        ability.action_cost = 4;
        ability.cooldown_turns = 9;
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        let target_id = state.location_enemies[&state.current_location_id][0].clone();

        apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: target_id,
            },
        );

        assert_eq!(state.turn_index, 4);
        assert_eq!(state.actor_ready_at("player"), 4);
        assert_eq!(state.ability_grants[0].ready_at_turn, 9);
        assert_eq!(
            super::super::abilities::ability_grant_cooldown_remaining(
                &state,
                &state.ability_grants[0]
            ),
            5
        );
    }

    #[test]
    fn blocked_move_preserves_player_readiness_and_world_time() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let mut state = generate_new_run(&bundle).state;
        state.actor_ready_at.insert("player".to_owned(), 0);

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::Move {
                destination: "laundry".to_owned(),
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::MovementBlocked { .. }]
        ));
        assert_eq!(state.turn_index, 0);
        assert_eq!(state.actor_ready_at("player"), 0);
    }

    #[test]
    fn encounter_actor_keeps_its_traversal_timing_when_controller_changes() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected fixture bundle");
        let enemy_id = bundle
            .locations
            .iter()
            .find_map(|location| location.enemies.first())
            .cloned()
            .expect("fixture includes a placed enemy");
        bundle
            .enemies
            .iter_mut()
            .find(|enemy| enemy.id == enemy_id)
            .expect("placed enemy template")
            .traversal_cost = 6;
        let destination = bundle
            .locations
            .iter()
            .find(|location| location.enemies.contains(&enemy_id))
            .and_then(|location| location.connections.first())
            .cloned()
            .expect("fixture encounter has an adjacent location");

        let mut player_controlled = generate_new_run(&bundle).state;
        assert_eq!(
            player_controlled.enemy_instances[&enemy_id].traversal_cost,
            6
        );
        player_controlled.player_entity_id = enemy_id.clone();
        assert_eq!(
            player_controlled
                .runtime_actor(&enemy_id)
                .expect("encounter actor projection")
                .actor
                .traversal_cost,
            6
        );
        assert_eq!(player_controlled.actor_traversal_cost(&enemy_id), 6);
        let controlled = apply_action(
            &mut player_controlled,
            &bundle,
            GameAction::Move {
                destination: destination.clone(),
            },
        );
        assert!(controlled.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved { origin_id, .. } if origin_id == &enemy_id
        )));
        assert_eq!(player_controlled.turn_index, 6);
        assert_eq!(player_controlled.actor_ready_at(&enemy_id), 6);

        let mut autonomous = generate_new_run(&bundle).state;
        let autonomous_outcome = apply_action(
            &mut autonomous,
            &bundle,
            GameAction::ActorMove {
                origin_id: enemy_id.clone(),
                destination,
            },
        );
        assert!(autonomous_outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved { origin_id, .. } if origin_id == &enemy_id
        )));
        assert_eq!(autonomous.turn_index, 6);
        assert_eq!(autonomous.actor_ready_at(&enemy_id), 6);
    }
}
