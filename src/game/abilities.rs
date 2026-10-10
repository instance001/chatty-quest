//! Immutable ability definitions are pack data; mutable possession belongs to grants.
//!
//! This is intentionally not a spell subsystem. A grant can belong to any eligible entity or
//! carried item, so skills, rituals, scrolls, staffs, and temporary training all share one
//! reducer-owned path.

use crate::data::datapacks::{
    AbilityEffect, AbilityStatusDuration, AbilityStatusTrigger, DatapackBundle,
};
use std::collections::BTreeSet;

use super::actions::{ActionOutcome, EncounterKind, GameEvent};
use super::discovery::reveal_current_connections;
use super::effects::evaluate_status_effects;
use super::queries::matches_name;
use super::spawned_ai::handle_actor_move;
use super::state::{
    AbilityGrant, AbilityGrantSource, EffectDuration, EffectTrigger, InstanceLifecycle, ItemOwner,
    RunState, StatusEffectInstance, StatusOwner,
};

pub enum AbilityIntent {
    Use { grant_id: String, target: String },
    Unlock { template_id: String },
}

/// Capability tags are the access gate for an immutable ability template. Possession is checked
/// separately, because a scroll may legitimately travel through actors who cannot use it.
pub fn actor_satisfies_ability_access(
    state: &RunState,
    template: &crate::data::datapacks::AbilityTemplate,
    origin_id: &str,
) -> bool {
    state.runtime_actor(origin_id).is_some_and(|actor| {
        template.required_tags.iter().all(|tag| {
            actor
                .tags
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(tag))
        })
    })
}

/// Presentation and command adapters use this same readiness predicate before selecting one
/// concrete grant. The reducer repeats the checks when committing the action, so an adapter can
/// never become an authority boundary.
pub fn ability_grant_unavailability_reason(
    state: &RunState,
    bundle: &DatapackBundle,
    grant: &AbilityGrant,
    origin_id: &str,
) -> Option<String> {
    if !state.ability_grant_is_accessible_to(grant, origin_id) {
        return Some("not possessed by this actor".to_owned());
    }
    if grant
        .expires_at_turn
        .is_some_and(|turn| state.turn_index >= turn)
    {
        return Some("expired".to_owned());
    }
    if grant.remaining_uses == Some(0) {
        return Some("spent".to_owned());
    }
    let cooldown_remaining = ability_grant_cooldown_remaining(state, grant);
    if cooldown_remaining > 0 {
        return Some(format!(
            "cooling down for {cooldown_remaining} more world turn(s)"
        ));
    }
    let Some(template) = bundle
        .abilities
        .iter()
        .find(|template| template.id == grant.template_id)
    else {
        return Some("template is unavailable".to_owned());
    };
    let Some(actor) = state.runtime_actor(origin_id) else {
        return Some("origin is not an active actor".to_owned());
    };
    if actor.location_id != state.current_location_id {
        return Some("origin is not at the current location".to_owned());
    }
    template
        .required_tags
        .iter()
        .find(|tag| {
            !actor
                .tags
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(tag))
        })
        .map(|tag| format!("missing required tag '{tag}'"))
}

/// Cooldowns use the authoritative world turn rather than per-turn mutable counters. A caller
/// can inspect this value without changing state, and old saves deserialize to ready at turn 0.
pub fn ability_grant_cooldown_remaining(state: &RunState, grant: &AbilityGrant) -> u64 {
    grant.ready_at_turn.saturating_sub(state.turn_index)
}

pub fn ability_grant_is_usable_by(
    state: &RunState,
    bundle: &DatapackBundle,
    grant: &AbilityGrant,
    origin_id: &str,
) -> bool {
    ability_grant_unavailability_reason(state, bundle, grant, origin_id).is_none()
}

/// Learning grants durable knowledge, not another copy of an ability the same owner already
/// knows forever. Finite, expiring, and item-owned grants deliberately do not satisfy this rule:
/// they remain consumable or transferable sources rather than a permanent unlock.
pub fn owner_has_permanent_ability(state: &RunState, owner: &ItemOwner, template_id: &str) -> bool {
    state.ability_grants.iter().any(|grant| {
        grant.owner == *owner
            && grant.template_id == template_id
            && grant.remaining_uses.is_none()
            && grant.expires_at_turn.is_none()
    })
}

/// XP establishes authored progression milestones; each reached positive milestone yields one
/// direct-unlock choice in its pool. Scroll study and other grant provenance are deliberately not
/// counted as choices, so found source material expands a school instead of consuming the build.
pub fn owner_has_unlock_choice_for_template(
    state: &RunState,
    bundle: &DatapackBundle,
    owner: &ItemOwner,
    template_id: &str,
) -> bool {
    let Some(template) = bundle
        .abilities
        .iter()
        .find(|template| template.id == template_id)
    else {
        return false;
    };
    let owner_id = owner.entity_id();
    let progress = state
        .progress_pools_for_actor(owner_id)
        .get(&template.progress_pool)
        .copied()
        .unwrap_or_default();
    if !template.learnable || template.required_xp <= 0 || progress < template.required_xp {
        return false;
    }
    let milestone_count = skill_level_for_progress(bundle, &template.progress_pool, progress);
    let spent_choices = state
        .ability_grants
        .iter()
        .filter(|grant| grant.owner == *owner && grant.source == AbilityGrantSource::Unlocked)
        .filter_map(|grant| {
            bundle
                .abilities
                .iter()
                .find(|candidate| candidate.id == grant.template_id)
        })
        .filter(|candidate| {
            candidate.learnable && candidate.progress_pool == template.progress_pool
        })
        .count();
    spent_choices < milestone_count as usize
}

/// A real skill level is one distinct positive direct-unlock milestone reached in a pool. This
/// deliberately keeps the automatic level count aligned with the existing bounded unlock-choice
/// contract; source-only abilities and duplicate templates cannot manufacture extra levels.
pub fn skill_level_for_progress(bundle: &DatapackBundle, pool: &str, progress: i32) -> u32 {
    bundle
        .abilities
        .iter()
        .filter(|candidate| {
            candidate.learnable
                && candidate.progress_pool == pool
                && candidate.required_xp > 0
                && candidate.required_xp <= progress
        })
        .map(|candidate| candidate.required_xp)
        .collect::<BTreeSet<_>>()
        .len() as u32
}

/// A permanent, currently usable passive grant may authorize studying item-held ability grants
/// whose immutable template shares one declared learning tag. The same tag vocabulary therefore
/// joins a skill school, its progression pool, and compatible source material without a
/// necromancy- or scroll-specific reducer branch.
pub fn owner_can_study_scroll_template(
    state: &RunState,
    bundle: &DatapackBundle,
    owner: &ItemOwner,
    template_id: &str,
) -> bool {
    let Some(scroll_template) = bundle
        .abilities
        .iter()
        .find(|template| template.id == template_id)
    else {
        return false;
    };
    let owner_id = owner.entity_id();
    state.ability_grants.iter().any(|grant| {
        grant.owner == *owner
            && grant.remaining_uses.is_none()
            && grant.expires_at_turn.is_none()
            && ability_grant_is_usable_by(state, bundle, grant, owner_id)
            && bundle
                .abilities
                .iter()
                .find(|template| template.id == grant.template_id)
                .is_some_and(|literacy| {
                    literacy.scroll_learning_tags.iter().any(|learning_tag| {
                        scroll_template
                            .tags
                            .iter()
                            .any(|scroll_tag| scroll_tag.eq_ignore_ascii_case(learning_tag))
                    })
                })
    })
}

/// Resolves `learn <carried source item>` before the ordinary immutable-template unlock adapter.
/// It names only a real controlled-actor-held item and one real item-owned grant; capability and tag checks
/// still belong to the reducer.
pub fn resolve_controlled_scroll_learning_intent(
    state: &RunState,
    input: &str,
) -> Option<Result<(String, String), String>> {
    let requested = input
        .trim()
        .to_ascii_lowercase()
        .strip_prefix("learn ")?
        .trim()
        .to_owned();
    if requested.is_empty() {
        return Some(Err(
            "Name a skill template or a carried source item to learn from.".to_owned(),
        ));
    }
    if !state.controlled_actor_can_carry_inventory() {
        return Some(Err(
            "The controlled actor lacks the inventory capability contract.".to_owned(),
        ));
    }
    let item = state
        .controlled_inventory()?
        .iter()
        .find(|item| matches_name(&requested, &item.id, &item.name))?;
    let grants = state
        .ability_grants
        .iter()
        .filter(|grant| {
            grant.owner == ItemOwner::Entity(item.instance_id.clone())
                && grant.source == AbilityGrantSource::Item
        })
        .collect::<Vec<_>>();
    match grants.as_slice() {
        [] => Some(Err(format!(
            "{} does not contain an ability source to study.",
            item.name
        ))),
        [grant] => Some(Ok((item.instance_id.clone(), grant.instance_id.clone()))),
        _ => Some(Err(format!(
            "{} contains multiple ability sources; select a specific source first.",
            item.name
        ))),
    }
}

/// Bounded natural-language seam for an existing non-player actor using an ability it actually
/// possesses. It resolves only a present runtime actor, an accessible concrete grant, and an
/// authored template; the reducer still validates and applies the effect.
pub fn resolve_actor_ability_intent(
    state: &RunState,
    bundle: &DatapackBundle,
    input: &str,
) -> Option<Result<(String, String, String), String>> {
    let normalized = input.trim().to_ascii_lowercase();
    if !normalized.contains("cast ") {
        return None;
    }
    // Claim only an explicit actor-cast grammar.  A player command such as
    // `cast Grave Bolt at a named creature` also contains an actor's name,
    // but its actor is the controlled player and must be left for the
    // controlled-ability parser below this adapter in the command pipeline.
    let (actor, request) = state
        .runtime_actor_ids_at(&state.current_location_id)
        .into_iter()
        .filter(|actor_id| actor_id != &state.player_entity_id)
        .filter_map(|actor_id| state.runtime_actor(&actor_id))
        .find_map(|actor| {
            let name = actor.name.to_ascii_lowercase();
            [
                format!("{name}, cast "),
                format!("{name} cast "),
                format!("ask {name} to cast "),
            ]
            .into_iter()
            .find_map(|prefix| {
                normalized
                    .strip_prefix(&prefix)
                    .map(|request| (actor.clone(), request.to_owned()))
            })
        })?;
    let (ability_name, target) = request
        .split_once(" at ")
        .or_else(|| request.split_once(" on "))
        .map_or((request.trim(), ""), |(ability, target)| {
            (ability.trim(), target.trim())
        });
    if ability_name.is_empty() {
        return Some(Err("Name an ability to cast.".to_owned()));
    }
    Some(
        state
            .ability_grants
            .iter()
            .filter(|grant| ability_grant_is_usable_by(state, bundle, grant, &actor.entity_id))
            .find(|grant| {
                bundle
                    .abilities
                    .iter()
                    .find(|template| template.id == grant.template_id)
                    .is_some_and(|template| {
                        template.id.eq_ignore_ascii_case(ability_name)
                            || template.name.eq_ignore_ascii_case(ability_name)
                    })
            })
            .map(|grant| {
                (
                    actor.entity_id,
                    grant.instance_id.clone(),
                    target.to_owned(),
                )
            })
            .ok_or_else(|| {
                format!(
                    "{} does not possess an accessible ability named '{}'.",
                    actor.name, ability_name
                )
            }),
    )
}

/// Converts a narrow controlled-actor phrase into an existing grant/template id. The narrator
/// never names a new spell or decides its target; this adapter only selects from current host
/// state.
pub fn resolve_controlled_ability_intent(
    state: &RunState,
    bundle: &DatapackBundle,
    input: &str,
) -> Option<Result<AbilityIntent, String>> {
    let normalized = input.trim().to_ascii_lowercase();
    if let Some(request) = normalized.strip_prefix("cast ") {
        let (ability_name, target) = request
            .split_once(" at ")
            .or_else(|| request.split_once(" on "))
            .map_or((request.trim(), ""), |(ability, target)| {
                (ability.trim(), target.trim())
            });
        if ability_name.is_empty() {
            return Some(Err("Name an ability to cast.".to_owned()));
        }
        return Some(
            state
                .ability_grants
                .iter()
                .filter(|grant| {
                    ability_grant_is_usable_by(state, bundle, grant, &state.player_entity_id)
                })
                .find(|grant| {
                    bundle
                        .abilities
                        .iter()
                        .find(|template| template.id == grant.template_id)
                        .is_some_and(|template| {
                            template.id.eq_ignore_ascii_case(ability_name)
                                || template.name.eq_ignore_ascii_case(ability_name)
                        })
                })
                .map(|grant| AbilityIntent::Use {
                    grant_id: grant.instance_id.clone(),
                    target: target.to_owned(),
                })
                .ok_or_else(|| format!("You do not possess an ability named '{ability_name}'.")),
        );
    }
    normalized.strip_prefix("learn ").map(|ability_name| {
        let ability_name = ability_name.trim();
        bundle
            .abilities
            .iter()
            .find(|template| {
                template.id.eq_ignore_ascii_case(ability_name)
                    || template.name.eq_ignore_ascii_case(ability_name)
            })
            .map(|template| AbilityIntent::Unlock {
                template_id: template.id.clone(),
            })
            .ok_or_else(|| format!("No ability template named '{ability_name}' is available."))
    })
}

pub fn handle_use_ability(
    state: &mut RunState,
    bundle: &DatapackBundle,
    origin_id: &str,
    grant_id: &str,
    target: &str,
) -> ActionOutcome {
    let Some(grant_index) = state
        .ability_grants
        .iter()
        .position(|grant| grant.instance_id == grant_id)
    else {
        return rejected("That ability grant does not exist.".to_owned());
    };
    let grant = state.ability_grants[grant_index].clone();
    if !state.ability_grant_is_accessible_to(&grant, origin_id) {
        return rejected("That actor does not possess the requested ability grant.".to_owned());
    }
    if grant
        .expires_at_turn
        .is_some_and(|turn| state.turn_index >= turn)
    {
        return rejected("That ability grant has expired.".to_owned());
    }
    if grant.remaining_uses == Some(0) {
        return rejected("That ability grant has no uses remaining.".to_owned());
    }
    if ability_grant_cooldown_remaining(state, &grant) > 0 {
        return rejected(format!(
            "That ability grant is cooling down for {} more world turn(s).",
            ability_grant_cooldown_remaining(state, &grant)
        ));
    }
    let Some(template) = bundle
        .abilities
        .iter()
        .find(|template| template.id == grant.template_id)
    else {
        return rejected(format!(
            "Ability grant '{}' references an unavailable template '{}'.",
            grant.instance_id, grant.template_id
        ));
    };
    let Some(origin) = state.runtime_actor(origin_id) else {
        return rejected("That ability origin is not an active runtime actor.".to_owned());
    };
    if origin
        .tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case("dead"))
    {
        return rejected(format!(
            "{} is incapacitated and cannot use abilities.",
            origin.name
        ));
    }
    if origin.location_id != state.current_location_id {
        return rejected("That ability origin is not at the current location.".to_owned());
    }
    if let Some(tag) = template.required_tags.iter().find(|tag| {
        !origin
            .tags
            .iter()
            .any(|owned| owned.eq_ignore_ascii_case(tag))
    }) {
        return rejected(format!(
            "{} lacks required access tag '{}'.",
            origin.name, tag
        ));
    }
    let Some(effect) = template.effect.as_ref() else {
        return rejected(format!(
            "{} is a passive ability and cannot be cast.",
            template.name
        ));
    };

    let mut restoration = None;
    let mut status_effect = None;
    let mut discovery = None;
    let mut movement_events = Vec::new();
    let mut movement_lines = Vec::new();
    let mut consumed_status_effect_ids = Vec::new();
    let (target_id, target_kind, damage, defeated) = match &effect {
        AbilityEffect::Damage { amount } => {
            let target_tags = match damage_target_tags(state, bundle, target) {
                Ok(tags) => tags,
                Err(reason) => return rejected(reason),
            };
            let context_tags = bundle
                .locations
                .iter()
                .find(|location| location.id == state.current_location_id)
                .map(|location| location.tags.as_slice())
                .unwrap_or_default();
            let effects = evaluate_status_effects(
                state,
                origin_id,
                EffectTrigger::Combat,
                &target_tags,
                context_tags,
            );
            consumed_status_effect_ids = effects.expired_instance_ids;
            match apply_damage_effect(
                state,
                target,
                amount.saturating_add(effects.additive_damage),
            ) {
                Ok(result) => result,
                Err(reason) => return rejected(reason),
            }
        }
        AbilityEffect::Healing {
            amount,
            required_target_tags,
        } => {
            let target_id = match resolve_healing_target(state, origin_id, target) {
                Ok(target_id) => target_id,
                Err(reason) => return rejected(reason),
            };
            if let Err(reason) = require_target_tags(
                state,
                &target_id,
                std::iter::once("alive").chain(required_target_tags.iter().map(String::as_str)),
            ) {
                return rejected(reason);
            }
            let (target_name, restored, remaining_hp) =
                match state.restore_runtime_actor_health(&target_id, *amount) {
                    Ok(result) => result,
                    Err(reason) => return rejected(reason),
                };
            restoration = Some((target_name, restored, remaining_hp));
            (target_id, EncounterKind::Actor, 0, false)
        }
        AbilityEffect::Revive {
            amount,
            required_target_tags,
        } => {
            let target_id = match resolve_healing_target(state, origin_id, target) {
                Ok(target_id) => target_id,
                Err(reason) => return rejected(reason),
            };
            if let Err(reason) = require_target_tags(
                state,
                &target_id,
                std::iter::once("dead").chain(required_target_tags.iter().map(String::as_str)),
            ) {
                return rejected(reason);
            }
            let (target_name, restored, remaining_hp) =
                match state.revive_runtime_actor_health(&target_id, *amount) {
                    Ok(result) => result,
                    Err(reason) => return rejected(reason),
                };
            restoration = Some((target_name, restored, remaining_hp));
            (target_id, EncounterKind::Actor, 0, false)
        }
        AbilityEffect::Move => {
            // A movement ability is an ability-shaped request for the one authoritative actor
            // movement primitive. It does not mutate location state itself: all route, lock,
            // capability, fog, and controlled-actor rules remain owned by that reducer.
            let movement = handle_actor_move(state, bundle, origin_id, target, None);
            let destination = movement.events.iter().find_map(|event| match event {
                GameEvent::Moved { to_location_id, .. }
                | GameEvent::ActorMoved { to_location_id, .. } => Some(to_location_id.clone()),
                _ => None,
            });
            let Some(destination) = destination else {
                // The movement reducer has already emitted its precise rejection. Do not spend a
                // grant, start a cooldown, or let the timing reducer charge a failed cast.
                return movement;
            };
            movement_events = movement.events;
            movement_lines = movement.lines;
            (destination, EncounterKind::Actor, 0, false)
        }
        AbilityEffect::RevealConnections => {
            let newly_known = match reveal_current_connections(state, bundle) {
                Ok(newly_known) => newly_known,
                Err(reason) => return rejected(reason),
            };
            discovery = Some(newly_known.len());
            (
                state.current_location_id.clone(),
                EncounterKind::Actor,
                0,
                false,
            )
        }
        AbilityEffect::Status {
            trigger,
            duration,
            additive_damage,
            additive_healing,
            timing_cost_delta,
            timing_action_kinds,
            blocked_action_kinds,
            required_target_tags,
            required_context_tags,
        } => {
            let instance_id = format!("ability_status_{}_{}", grant.instance_id, state.turn_index);
            let trigger = match trigger {
                AbilityStatusTrigger::Combat => EffectTrigger::Combat,
                AbilityStatusTrigger::Movement => EffectTrigger::Movement,
                AbilityStatusTrigger::Inspect => EffectTrigger::Inspect,
                AbilityStatusTrigger::Wait => EffectTrigger::Wait,
                AbilityStatusTrigger::Failure => EffectTrigger::Failure,
            };
            let duration = match duration {
                AbilityStatusDuration::Scene => EffectDuration::Scene,
                AbilityStatusDuration::Run => EffectDuration::Run,
                AbilityStatusDuration::UntilUsed => EffectDuration::UntilUsed,
            };
            state.status_effects.push(StatusEffectInstance {
                instance_id: instance_id.clone(),
                template_id: template.id.clone(),
                // A cast status belongs to the caster, not the whole scene. Scene-wide effects
                // remain a separate host-owned status scope for hazards and authored locations.
                owner: StatusOwner::Entity,
                owner_id: Some(origin_id.to_owned()),
                trigger: trigger.clone(),
                duration: duration.clone(),
                additive_damage: *additive_damage,
                additive_healing: *additive_healing,
                timing_cost_delta: *timing_cost_delta,
                timing_action_kinds: timing_action_kinds.clone(),
                blocked_action_kinds: blocked_action_kinds.clone(),
                required_target_tags: required_target_tags.clone(),
                required_context_tags: required_context_tags.clone(),
                lifecycle: InstanceLifecycle::Active,
            });
            status_effect = Some((instance_id, trigger, duration));
            (
                state.current_location_id.clone(),
                EncounterKind::Actor,
                0,
                false,
            )
        }
    };

    let grant = &mut state.ability_grants[grant_index];
    if let Some(remaining) = &mut grant.remaining_uses {
        *remaining = remaining.saturating_sub(1);
    }
    grant.ready_at_turn = state.turn_index.saturating_add(template.cooldown_turns);
    let remaining_uses = grant.remaining_uses;
    let pool = template.progress_pool.clone();
    // Progress belongs to the actor currently controlled by this run. Other actors may use the
    // exact same ability/grant machinery, but cannot mutate that actor's progression ledger
    // merely because they were asked to cast.
    let awarded = if origin_id == state.player_entity_id {
        template.xp_on_success.max(0)
    } else {
        0
    };
    let mut events = vec![GameEvent::AbilityUsed {
        origin_id: origin_id.to_owned(),
        grant_id: grant_id.to_owned(),
        template_id: template.id.clone(),
        target_id: target_id.clone(),
        effect: effect.label().to_owned(),
        remaining_uses,
    }];
    if template.cooldown_turns > 0 {
        events.push(GameEvent::AbilityCooldownSet {
            origin_id: origin_id.to_owned(),
            grant_id: grant_id.to_owned(),
            template_id: template.id.clone(),
            ready_at_turn: grant.ready_at_turn,
        });
    }
    if matches!(effect, AbilityEffect::Damage { .. }) {
        events.push(GameEvent::AttackResolved {
            origin_id: origin_id.to_owned(),
            target_id: target_id.clone(),
            target_kind,
            damage,
            defeated,
        });
    }
    events.extend(
        consumed_status_effect_ids
            .into_iter()
            .map(|instance_id| GameEvent::StatusEffectExpired { instance_id }),
    );
    if let Some((instance_id, trigger, duration)) = status_effect {
        events.push(GameEvent::StatusEffectApplied {
            origin_id: origin_id.to_owned(),
            instance_id,
            trigger,
            duration,
        });
    }
    if let Some(count) = discovery {
        events.push(GameEvent::ConnectionsRevealed {
            origin_id: origin_id.to_owned(),
            location_id: state.current_location_id.clone(),
            count,
        });
    }
    if let Some((_, restored, remaining_hp)) = restoration {
        events.push(GameEvent::HealthRestored {
            origin_id: origin_id.to_owned(),
            target_id: target_id.clone(),
            amount: restored,
            remaining_hp,
        });
    }
    events.extend(movement_events);
    let progress_outcome =
        (awarded > 0).then(|| award_progress(state, bundle, origin_id, &pool, awarded));
    if let Some(progress_outcome) = &progress_outcome {
        events.extend(progress_outcome.events.clone());
    }
    let uses = remaining_uses.map_or("infinite".to_owned(), |uses| uses.to_string());
    let mut lines = vec![format!(
        "{} uses {} ({} uses remaining).",
        origin.name, template.name, uses
    )];
    lines.extend(movement_lines);
    if let Some(progress_outcome) = progress_outcome {
        lines.extend(progress_outcome.lines);
    }
    ActionOutcome { events, lines }
}

/// The one reducer primitive for controlled-actor progression. The save-compatible pool ledger
/// stays on the run for now, while health rewards apply to the actual actor that earned them.
pub fn award_progress(
    state: &mut RunState,
    bundle: &DatapackBundle,
    recipient_id: &str,
    pool: &str,
    amount: i32,
) -> ActionOutcome {
    if recipient_id != state.player_entity_id {
        return rejected(
            "Progress can currently be awarded only to the controlled actor.".to_owned(),
        );
    }
    if amount <= 0 {
        return rejected("Progress awards must be positive.".to_owned());
    }
    if !bundle
        .abilities
        .iter()
        .any(|ability| ability.progress_pool == pool)
    {
        return rejected(format!("Unknown progress pool '{pool}'."));
    }
    let previous_total = state
        .progress_pools_for_actor(recipient_id)
        .get(pool)
        .copied()
        .unwrap_or_default();
    let previous_level = skill_level_for_progress(bundle, pool, previous_total);
    let total = {
        let value = state
            .progress_pools_for_actor_mut(recipient_id)
            .entry(pool.to_owned())
            .or_default();
        *value = value.saturating_add(amount);
        *value
    };
    let level = skill_level_for_progress(bundle, pool, total);
    let gained_levels = level.saturating_sub(previous_level);
    let hp_increase = i32::try_from(bundle.rules.hp_per_skill_level)
        .unwrap_or(i32::MAX)
        .saturating_mul(i32::try_from(gained_levels).unwrap_or(i32::MAX));
    let maximum_hp = if hp_increase > 0 {
        match state.increase_runtime_actor_max_health(recipient_id, hp_increase) {
            Ok((_, maximum)) => Some(maximum),
            Err(reason) => return rejected(reason),
        }
    } else {
        None
    };
    let mut events = vec![GameEvent::ProgressAwarded {
        origin_id: recipient_id.to_owned(),
        pool: pool.to_owned(),
        amount,
        total,
    }];
    for reached_level in (previous_level + 1)..=level {
        events.push(GameEvent::SkillLevelGained {
            origin_id: recipient_id.to_owned(),
            pool: pool.to_owned(),
            level: reached_level,
        });
    }
    if let Some(max_hp) = maximum_hp {
        events.push(GameEvent::MaxHpIncreased {
            origin_id: recipient_id.to_owned(),
            amount: hp_increase,
            max_hp,
        });
    }
    let mut lines = vec![format!("{amount} {pool} XP awarded; total {total}.")];
    if gained_levels > 0 {
        lines.push(format!("{pool} reached skill level {level}."));
    }
    if let Some(max_hp) = maximum_hp {
        lines.push(format!(
            "Maximum HP increased by {hp_increase} to {max_hp}."
        ));
    }
    ActionOutcome { events, lines }
}

fn resolve_healing_target(
    state: &RunState,
    origin_id: &str,
    target: &str,
) -> Result<String, String> {
    let wanted = target.trim().to_ascii_lowercase();
    if wanted.is_empty() || wanted == "self" {
        return Ok(origin_id.to_owned());
    }
    if matches!(wanted.as_str(), "player" | "you" | "me") {
        return Ok(state.player_entity_id.clone());
    }
    state
        .runtime_actor_ids_at(&state.current_location_id)
        .into_iter()
        .find(|candidate| {
            candidate.eq_ignore_ascii_case(&wanted)
                || state
                    .runtime_actor(candidate)
                    .is_some_and(|actor| actor.name.eq_ignore_ascii_case(&wanted))
        })
        .ok_or_else(|| format!("No valid actor named '{target}' is present."))
}

fn require_target_tags<'a>(
    state: &RunState,
    target_id: &str,
    required_tags: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    let target = state
        .runtime_actor(target_id)
        .ok_or_else(|| "That target is not an active runtime actor.".to_owned())?;
    let required_tags = required_tags.into_iter().collect::<Vec<_>>();
    if let Some(tag) = required_tags.iter().find(|required| {
        !target
            .tags
            .iter()
            .any(|actual| actual.eq_ignore_ascii_case(required))
    }) {
        return Err(format!(
            "{} lacks required target tag '{}'.",
            target.name, tag
        ));
    }
    Ok(())
}

pub fn unlock_ability(
    state: &mut RunState,
    bundle: &DatapackBundle,
    owner: ItemOwner,
    template_id: &str,
) -> ActionOutcome {
    let Some(template) = bundle
        .abilities
        .iter()
        .find(|entry| entry.id == template_id)
    else {
        return rejected(format!("Unknown ability template '{template_id}'."));
    };
    if !template.learnable {
        return rejected(format!(
            "{} cannot be learned from progression.",
            template.name
        ));
    }
    let owner_id = owner.entity_id().to_owned();
    if owner_id != state.player_entity_id {
        return rejected(
            "Ability unlocks currently belong to the controlled actor's progression ledger."
                .to_owned(),
        );
    }
    let Some(actor) = state.runtime_actor(&owner_id) else {
        return rejected("Ability unlock owner is not an active runtime actor.".to_owned());
    };
    if let Some(tag) = template.required_tags.iter().find(|tag| {
        !actor
            .tags
            .iter()
            .any(|owned| owned.eq_ignore_ascii_case(tag))
    }) {
        return rejected(format!(
            "{} lacks required access tag '{}'.",
            actor.name, tag
        ));
    }
    let progress = *state
        .progress_pools_for_actor(&owner_id)
        .get(&template.progress_pool)
        .unwrap_or(&0);
    if progress < template.required_xp {
        return rejected(format!(
            "{} requires {} {} XP; current total is {}.",
            template.name, template.required_xp, template.progress_pool, progress
        ));
    }
    if owner_has_permanent_ability(state, &owner, &template.id) {
        return rejected(format!("{} is already permanently known.", template.name));
    }
    if !owner_has_unlock_choice_for_template(state, bundle, &owner, &template.id) {
        return rejected(format!(
            "{} has no unspent {} XP unlock choice at this milestone.",
            template.progress_pool, template.progress_pool
        ));
    }
    grant_ability(
        state,
        bundle,
        owner,
        template_id,
        // Learning is a permanent unlock. Finite charges and expiry belong to a particular
        // source grant (scroll, staff, reward, temporary effect), not the shared template.
        None,
        None,
        AbilityGrantSource::Unlocked,
    )
}

/// Consumes one controlled-actor-carried, item-owned grant and turns its immutable template into
/// permanent knowledge for that same owner. This is intentionally a separate action from direct XP unlocks: the
/// literacy template decides whether the physical source's tags are legible, while the source
/// template decides whether study is allowed at all.
pub fn learn_ability_from_item(
    state: &mut RunState,
    bundle: &DatapackBundle,
    learner_id: &str,
    item_id: &str,
    grant_id: &str,
) -> ActionOutcome {
    if learner_id != state.player_entity_id {
        return rejected(
            "Ability study currently belongs to the controlled actor's progression ledger."
                .to_owned(),
        );
    }
    if !state.controlled_actor_can_carry_inventory() {
        return rejected(
            "The controlled actor lacks the inventory capability contract.".to_owned(),
        );
    }
    let owner = state.controlled_inventory_owner();
    let Some(item) = state
        .controlled_inventory()
        .into_iter()
        .flatten()
        .find(|item| item.id == item_id || item.instance_id == item_id)
        .cloned()
    else {
        return rejected("That source item is not in the controlled actor's inventory.".to_owned());
    };
    let Some(grant) = state
        .ability_grants
        .iter()
        .find(|grant| grant.instance_id == grant_id)
        .cloned()
    else {
        return rejected("That source ability grant does not exist.".to_owned());
    };
    if grant.source != AbilityGrantSource::Item
        || grant.owner != ItemOwner::Entity(item.instance_id.clone())
    {
        return rejected(
            "That ability grant is not contained by the requested source item.".to_owned(),
        );
    }
    if let Some(reason) = ability_grant_unavailability_reason(state, bundle, &grant, learner_id) {
        return rejected(format!("That source cannot be studied: {reason}."));
    }
    let Some(template) = bundle
        .abilities
        .iter()
        .find(|template| template.id == grant.template_id)
    else {
        return rejected("The source ability template is unavailable.".to_owned());
    };
    if !template.scroll_learnable {
        return rejected(format!(
            "{} cannot be learned from a source item.",
            template.name
        ));
    }
    if owner_has_permanent_ability(state, &owner, &template.id) {
        return rejected(format!("{} is already permanently known.", template.name));
    }
    if !owner_can_study_scroll_template(state, bundle, &owner, &template.id) {
        return rejected(format!(
            "No permanent learned literacy matches {}'s skill tags.",
            template.name
        ));
    }

    let consumed = match state.consume_owned_item(&item.instance_id, &owner) {
        Ok(item) => item,
        Err(reason) => return rejected(reason),
    };
    state
        .ability_grants
        .retain(|candidate| candidate.owner != ItemOwner::Entity(consumed.instance_id.clone()));
    let mut outcome = grant_ability(
        state,
        bundle,
        owner,
        &template.id,
        None,
        None,
        AbilityGrantSource::Studied,
    );
    // All failure conditions are resolved before consumption. Keep the guard so a future change
    // cannot silently destroy the source item if grant materialization ever becomes fallible.
    if outcome
        .events
        .iter()
        .any(|event| matches!(event, GameEvent::AbilityGranted { .. }))
    {
        outcome.events.insert(
            0,
            GameEvent::ItemConsumed {
                origin_id: learner_id.to_owned(),
                item_id: consumed.instance_id.clone(),
                template_id: consumed.effective_template_id().to_owned(),
                reason: format!("studied into {}", template.id),
            },
        );
        outcome.lines.insert(
            0,
            format!(
                "You study the {} until its working becomes your own.",
                consumed.name
            ),
        );
    }
    outcome
}

/// Reducer implementation for all ways a concrete owner acquires an ability. The caller decides
/// that a reward, trainer, item, or temporary effect is legitimate; this shared primitive only
/// validates the canonical owner/template and creates the independent mutable grant.
pub fn grant_ability(
    state: &mut RunState,
    bundle: &DatapackBundle,
    owner: ItemOwner,
    template_id: &str,
    remaining_uses: Option<u32>,
    duration_turns: Option<u64>,
    source: AbilityGrantSource,
) -> ActionOutcome {
    if remaining_uses == Some(0) {
        return rejected("Ability grant remaining uses must be positive when finite.".to_owned());
    }
    if duration_turns == Some(0) {
        return rejected("Ability grant duration must be positive when finite.".to_owned());
    }
    let Some(template) = bundle
        .abilities
        .iter()
        .find(|entry| entry.id == template_id)
    else {
        return rejected(format!("Unknown ability template '{template_id}'."));
    };
    let owner_id = match &owner {
        ItemOwner::Player => "player".to_owned(),
        ItemOwner::Entity(entity_id) => {
            if state.runtime_actor(entity_id).is_none()
                && state.find_runtime_item(entity_id).is_none()
            {
                return rejected(format!(
                    "Ability grant owner '{entity_id}' is not an active actor or item."
                ));
            }
            entity_id.clone()
        }
        ItemOwner::Location(location_id) => {
            return rejected(format!(
                "Location '{location_id}' cannot directly own an ability grant."
            ));
        }
    };
    let grant = AbilityGrant {
        instance_id: state.next_ability_grant_id(&template.id),
        template_id: template.id.clone(),
        owner,
        remaining_uses,
        // A successful reducer action advances the clock after this function returns. Account for
        // that common action boundary so `duration_turns = 3` stays usable for three full turns.
        expires_at_turn: duration_turns
            .map(|turns| state.turn_index.saturating_add(turns).saturating_add(1)),
        ready_at_turn: 0,
        source,
    };
    let grant_id = grant.instance_id.clone();
    state.ability_grants.push(grant);
    ActionOutcome {
        events: vec![GameEvent::AbilityGranted {
            owner_id,
            grant_id,
            template_id: template.id.clone(),
            source,
        }],
        lines: vec![format!("Granted ability: {}.", template.name)],
    }
}

/// Retires only grants whose explicit lifetime has elapsed. Finite uses remain visible at zero so
/// a carried scroll or staff can still be inspected; expiry is a different lifecycle transition.
pub fn expire_ability_grants(state: &mut RunState) -> Vec<GameEvent> {
    let mut expired = Vec::new();
    state.ability_grants.retain(|grant| {
        let is_expired = grant
            .expires_at_turn
            .is_some_and(|turn| state.turn_index >= turn);
        if is_expired {
            expired.push(GameEvent::AbilityGrantExpired {
                owner_id: grant.owner.entity_id().to_owned(),
                grant_id: grant.instance_id.clone(),
                template_id: grant.template_id.clone(),
            });
        }
        !is_expired
    });
    expired
}

fn apply_damage_effect(
    state: &mut RunState,
    target: &str,
    amount: i32,
) -> Result<(String, EncounterKind, i32, bool), String> {
    let location_id = state.current_location_id.clone();
    let target_id = super::combat::first_matching_hostile(
        state,
        (!target.trim().is_empty()).then_some(target),
        &location_id,
    )
    .ok_or_else(|| "No valid hostile target is present for that ability.".to_owned())?;
    let (kind, _name, defeated) = super::combat::damage_hostile_target(state, &target_id, amount)?;
    Ok((target_id, kind, amount.max(1), defeated))
}

/// Validates the same hostile target envelope as `apply_damage_effect`, but without mutation so
/// an applicable combat status can be evaluated before a cast commits damage or consumes an
/// `until_used` instance.
fn damage_target_tags(
    state: &RunState,
    bundle: &DatapackBundle,
    target: &str,
) -> Result<Vec<String>, String> {
    let target_id = super::combat::first_matching_hostile(
        state,
        (!target.trim().is_empty()).then_some(target),
        &state.current_location_id,
    )
    .ok_or_else(|| "No valid hostile target is present for that ability.".to_owned())?;
    Ok(super::combat::hostile_target_tags(
        state, bundle, &target_id,
    ))
}

fn rejected(reason: String) -> ActionOutcome {
    ActionOutcome {
        events: vec![GameEvent::ActionRejected {
            reason: reason.clone(),
        }],
        lines: vec![reason],
    }
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::{
        AbilityEffect, AbilityStatusDuration, AbilityStatusTrigger, AbilityTemplate,
        load_datapack_bundle_by_folder,
    };
    use crate::game::actions::EncounterKind;
    use crate::game::{GameAction, GameEvent, apply_action, generate_new_run};

    use super::{
        AbilityGrantSource, ItemOwner, ability_grant_cooldown_remaining,
        ability_grant_unavailability_reason, award_progress, handle_use_ability,
        owner_has_permanent_ability, owner_has_unlock_choice_for_template,
        resolve_actor_ability_intent, resolve_controlled_ability_intent,
        resolve_controlled_scroll_learning_intent,
    };
    use crate::game::state::{
        AbilityGrant, ActionTimingKind, ActorDisposition, ActorState, EffectDuration,
        EffectTrigger, InstanceLifecycle, NpcInstance, StatusEffectInstance, StatusOwner,
    };

    #[test]
    fn immutable_template_has_independent_per_grant_charges_and_awards_progress() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let template_id = "grave_bolt".to_owned();
        let starter_id = state.ability_grants[0].instance_id.clone();
        state.ability_grants[0].remaining_uses = Some(2);
        state.ability_grants.push(AbilityGrant {
            instance_id: "separate_scroll_grant".to_owned(),
            template_id: template_id.clone(),
            owner: ItemOwner::Player,
            remaining_uses: Some(3),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Item,
        });
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: starter_id.clone(),
                target: target_id.clone(),
            },
        );

        assert_eq!(state.enemy_hp[&target_id], hp_before - 2);
        assert_eq!(
            state
                .ability_grants
                .iter()
                .find(|grant| grant.instance_id == starter_id)
                .and_then(|grant| grant.remaining_uses),
            Some(1)
        );
        assert_eq!(
            state
                .ability_grants
                .iter()
                .find(|grant| grant.instance_id == "separate_scroll_grant")
                .and_then(|grant| grant.remaining_uses),
            Some(3),
            "one owner's cast cannot mutate another grant of the same template"
        );
        assert_eq!(state.progress_pools.get("necromancy"), Some(&1));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityUsed { template_id: actual, remaining_uses: Some(1), .. }
                if actual == &template_id
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ProgressAwarded { pool, amount: 1, total: 1, .. }
                if pool == "necromancy"
        )));
    }

    #[test]
    fn legacy_player_grants_do_not_follow_a_non_player_controller() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let player_grant = state
            .ability_grants
            .iter()
            .find(|grant| grant.owner == ItemOwner::Player)
            .expect("starter grant")
            .clone();
        assert!(state.ability_grant_is_accessible_to(&player_grant, "player"));

        let actor_id = "ownership_fixture".to_owned();
        let location_id = state.current_location_id.clone();
        state
            .location_npcs
            .entry(location_id.clone())
            .or_default()
            .push(NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "ownership_fixture".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Ownership fixture".to_owned(),
                description: "Must not inherit the player's grants.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["can_necromancy".to_owned()],
                disposition: ActorDisposition::Friendly,
                actor: ActorState::configured(Some(10), Some(1), false, false, false, false),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            });
        state.player_entity_id = actor_id.clone();

        assert!(!state.ability_grant_is_accessible_to(&player_grant, &actor_id));
        assert_eq!(player_grant.owner.entity_id(), "player");
    }

    #[test]
    fn controlled_ability_command_uses_the_current_actors_grant_and_progress_pool() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let actor_id = "controlled_caster_fixture".to_owned();
        let location_id = state.current_location_id.clone();
        state
            .location_npcs
            .entry(location_id.clone())
            .or_default()
            .push(NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "controlled_caster_fixture".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Controlled caster fixture".to_owned(),
                description: "Owns a real ability grant.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["can_necromancy".to_owned()],
                disposition: ActorDisposition::Friendly,
                actor: ActorState::configured(Some(10), None, false, false, false, false),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            });
        let grant_id = "controlled_grave_bolt".to_owned();
        state.ability_grants.push(AbilityGrant {
            instance_id: grant_id.clone(),
            template_id: "grave_bolt".to_owned(),
            owner: ItemOwner::Entity(actor_id.clone()),
            remaining_uses: None,
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Granted,
        });
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];
        state.player_entity_id = actor_id.clone();

        let intent = resolve_controlled_ability_intent(
            &state,
            &bundle,
            &format!("cast grave bolt at {target_id}"),
        )
        .expect("cast command should be recognized")
        .expect("controlled grant should resolve");
        let super::AbilityIntent::Use {
            grant_id: resolved_grant,
            target,
        } = intent
        else {
            panic!("cast command must resolve to a concrete use");
        };
        assert_eq!(resolved_grant, grant_id);

        let controlled_origin = state.player_entity_id.clone();
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: controlled_origin,
                grant_id: resolved_grant,
                target,
            },
        );

        assert_eq!(state.enemy_hp[&target_id], hp_before - 2);
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityUsed { origin_id, grant_id: actual, .. }
                if origin_id == &actor_id && actual == &grant_id
        )));
        assert_eq!(state.progress_pools.get("necromancy"), None);
        assert_eq!(
            state.progress_pools_for_actor(&actor_id).get("necromancy"),
            Some(&1)
        );
    }

    #[test]
    fn grant_readiness_explains_the_first_host_gate_that_blocks_use() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let grant = state.ability_grants[0].clone();

        state.player_tags.retain(|tag| tag != "can_necromancy");
        assert_eq!(
            ability_grant_unavailability_reason(&state, &bundle, &grant, "player"),
            Some("missing required tag 'can_necromancy'".to_owned())
        );

        let mut spent = grant;
        spent.remaining_uses = Some(0);
        assert_eq!(
            ability_grant_unavailability_reason(&state, &bundle, &spent, "player"),
            Some("spent".to_owned())
        );
    }

    #[test]
    fn intent_selects_a_usable_duplicate_grant_not_a_spent_one() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let spent_id = state.ability_grants[0].instance_id.clone();
        state.ability_grants[0].remaining_uses = Some(0);
        state.ability_grants.push(AbilityGrant {
            instance_id: "usable_grave_bolt".to_owned(),
            template_id: "grave_bolt".to_owned(),
            owner: ItemOwner::Player,
            remaining_uses: Some(1),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Temporary,
        });

        let intent =
            resolve_controlled_ability_intent(&state, &bundle, "cast grave bolt at shambler")
                .expect("ability command should be recognized")
                .expect("a usable duplicate grant should resolve");
        assert!(matches!(
            intent,
            super::AbilityIntent::Use { grant_id, .. } if grant_id == "usable_grave_bolt"
        ));
        assert!(
            state
                .ability_grants
                .iter()
                .any(|grant| grant.instance_id == spent_id && grant.remaining_uses == Some(0))
        );
    }

    #[test]
    fn player_cast_at_a_named_actor_is_not_misread_as_an_actor_cast() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let state = generate_new_run(&bundle).state;
        let enemy_id = state.location_enemies[&state.current_location_id][0].clone();
        let enemy_name = state.enemy_instances[&enemy_id].name.clone();
        let command = format!("cast grave bolt at {enemy_name}");

        assert!(
            resolve_actor_ability_intent(&state, &bundle, &command).is_none(),
            "an actor mentioned only as the target must not claim the actor-cast parser"
        );
        assert!(matches!(
            resolve_controlled_ability_intent(&state, &bundle, &command),
            Some(Ok(super::AbilityIntent::Use { .. }))
        ));
    }

    #[test]
    fn unlock_is_threshold_gated_by_the_same_template_and_tags() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let locked = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "bone_lance".to_owned(),
            },
        );
        assert!(matches!(
            locked.events.as_slice(),
            [GameEvent::ActionRejected { .. }]
        ));
        state.progress_pools.insert("necromancy".to_owned(), 10);
        let unlocked = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "bone_lance".to_owned(),
            },
        );
        assert!(matches!(
            unlocked.events.as_slice(),
            [GameEvent::AbilityGranted { template_id, .. }] if template_id == "bone_lance"
        ));
        assert!(
            state
                .ability_grants
                .iter()
                .any(|grant| grant.template_id == "bone_lance" && grant.remaining_uses.is_none())
        );
    }

    #[test]
    fn learned_unlock_remains_permanent_when_the_template_has_finite_defaults() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let template = bundle
            .abilities
            .iter_mut()
            .find(|template| template.id == "bone_lance")
            .expect("fixture ability");
        template.default_uses = Some(2);
        template.duration_turns = Some(3);
        let mut state = generate_new_run(&bundle).state;
        state.progress_pools.insert("necromancy".to_owned(), 10);

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "bone_lance".to_owned(),
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::AbilityGranted {
                source: AbilityGrantSource::Unlocked,
                ..
            }]
        ));
        let grant = state
            .ability_grants
            .iter()
            .find(|grant| {
                grant.template_id == "bone_lance" && grant.source == AbilityGrantSource::Unlocked
            })
            .expect("learned grant");
        assert_eq!(grant.remaining_uses, None);
        assert_eq!(grant.expires_at_turn, None);
    }

    #[test]
    fn permanent_knowledge_blocks_a_duplicate_unlock_but_a_finite_copy_does_not() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;

        let duplicate_starter = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "grave_bolt".to_owned(),
            },
        );
        assert!(matches!(
            duplicate_starter.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("already permanently known")
        ));

        state.progress_pools.insert("necromancy".to_owned(), 10);
        state.ability_grants.push(AbilityGrant {
            instance_id: "finite_bone_lance_copy".to_owned(),
            template_id: "bone_lance".to_owned(),
            owner: ItemOwner::Player,
            remaining_uses: Some(1),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Granted,
        });
        let permanent_unlock = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "bone_lance".to_owned(),
            },
        );
        assert!(matches!(
            permanent_unlock.events.as_slice(),
            [GameEvent::AbilityGranted {
                source: AbilityGrantSource::Unlocked,
                ..
            }]
        ));
    }

    #[test]
    fn source_only_template_can_be_granted_but_not_learned() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;

        let grant = apply_action(
            &mut state,
            &bundle,
            GameAction::GrantAbility {
                owner: ItemOwner::Player,
                template_id: "spectral_spark".to_owned(),
                remaining_uses: Some(1),
                duration_turns: None,
                source: AbilityGrantSource::Granted,
            },
        );
        assert!(matches!(
            grant.events.as_slice(),
            [GameEvent::AbilityGranted { template_id, .. }] if template_id == "spectral_spark"
        ));

        let learn = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "spectral_spark".to_owned(),
            },
        );
        assert!(matches!(
            learn.events.as_slice(),
            [GameEvent::ActionRejected { reason }]
                if reason.contains("cannot be learned from progression")
        ));
    }

    #[test]
    fn matching_permanent_literacy_consumes_a_real_scroll_into_permanent_knowledge() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let mut scroll = state
            .world_item_instances
            .values()
            .find(|item| item.id == "grave_scroll")
            .expect("authored scroll")
            .clone();
        let source_location = scroll.location_id.take().expect("scroll location");
        state.world_item_instances.remove(&scroll.instance_id);
        state
            .location_items
            .get_mut(&source_location)
            .expect("scroll location inventory")
            .retain(|id| id != &scroll.instance_id);
        state.inventory.push(scroll.clone());
        let source_grant_id = state
            .ability_grants
            .iter()
            .find(|grant| grant.owner == ItemOwner::Entity(scroll.instance_id.clone()))
            .expect("scroll-owned grant")
            .instance_id
            .clone();

        let without_literacy = apply_action(
            &mut state,
            &bundle,
            GameAction::LearnAbilityFromItem {
                learner_id: "player".to_owned(),
                item_id: scroll.instance_id.clone(),
                grant_id: source_grant_id.clone(),
            },
        );
        assert!(matches!(
            without_literacy.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("No permanent learned literacy")
        ));
        assert!(state.find_runtime_item(&scroll.instance_id).is_some());

        state.progress_pools.insert("necromancy".to_owned(), 10);
        let literacy = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "necromantic_literacy".to_owned(),
            },
        );
        assert!(matches!(
            literacy.events.as_slice(),
            [GameEvent::AbilityGranted { template_id, .. }] if template_id == "necromantic_literacy"
        ));
        let exhausted_choice = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Player,
                template_id: "bone_lance".to_owned(),
            },
        );
        assert!(matches!(
            exhausted_choice.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("no unspent necromancy XP unlock choice")
        ));
        assert!(matches!(
            resolve_controlled_scroll_learning_intent(&state, "learn salt-stained scroll"),
            Some(Ok((item_id, grant_id)))
                if item_id == scroll.instance_id && grant_id == source_grant_id
        ));

        let learned = apply_action(
            &mut state,
            &bundle,
            GameAction::LearnAbilityFromItem {
                learner_id: "player".to_owned(),
                item_id: scroll.instance_id.clone(),
                grant_id: source_grant_id,
            },
        );
        assert!(matches!(
            learned.events.as_slice(),
            [GameEvent::ItemConsumed { item_id, .. }, GameEvent::AbilityGranted {
                template_id,
                source: AbilityGrantSource::Studied,
                ..
            }] if item_id == &scroll.instance_id && template_id == "spectral_spark"
        ));
        assert!(state.find_runtime_item(&scroll.instance_id).is_none());
        assert!(
            !state
                .ability_grants
                .iter()
                .any(|grant| grant.owner == ItemOwner::Entity(scroll.instance_id.clone()))
        );
        assert!(owner_has_permanent_ability(
            &state,
            &ItemOwner::Player,
            "spectral_spark"
        ));
    }

    #[test]
    fn controlled_actor_studies_scrolls_and_unlocks_from_its_own_progression_ledger() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let actor_id = "scroll_study_fixture".to_owned();
        let location_id = state.current_location_id.clone();
        let mut scroll = state
            .world_item_instances
            .values()
            .find(|item| item.id == "grave_scroll")
            .expect("authored scroll")
            .clone();
        let source_location = scroll.location_id.take().expect("scroll location");
        state.world_item_instances.remove(&scroll.instance_id);
        let mut actor = ActorState::configured(Some(10), Some(1), false, false, false, false);
        actor.add_declared_capabilities(&["inventory".to_owned()]);
        state
            .location_items
            .get_mut(&source_location)
            .expect("scroll location inventory")
            .retain(|id| id != &scroll.instance_id);
        state
            .location_npcs
            .entry(location_id.clone())
            .or_default()
            .push(NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "scroll_study_fixture".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Scroll study fixture".to_owned(),
                description: "Owns its own knowledge and source item.".to_owned(),
                faction: "test".to_owned(),
                tags: vec!["can_necromancy".to_owned()],
                disposition: ActorDisposition::Friendly,
                actor,
                quest_objective_ids: Vec::new(),
                inventory: vec![scroll.clone()],
                equipped_item_id: None,
            });
        state.player_entity_id = actor_id.clone();
        state
            .progress_pools_for_actor_mut(&actor_id)
            .insert("necromancy".to_owned(), 10);
        let owner = ItemOwner::Entity(actor_id.clone());
        let source_grant_id = state
            .ability_grants
            .iter()
            .find(|grant| grant.owner == ItemOwner::Entity(scroll.instance_id.clone()))
            .expect("scroll-owned grant")
            .instance_id
            .clone();

        let literacy = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: owner.clone(),
                template_id: "necromantic_literacy".to_owned(),
            },
        );
        assert!(matches!(
            literacy.events.as_slice(),
            [GameEvent::AbilityGranted { owner_id, template_id, .. }]
                if owner_id == &actor_id && template_id == "necromantic_literacy"
        ));
        assert!(state.progress_pools.is_empty());
        assert!(matches!(
            resolve_controlled_scroll_learning_intent(&state, "learn salt-stained scroll"),
            Some(Ok((item_id, grant_id)))
                if item_id == scroll.instance_id && grant_id == source_grant_id
        ));

        let learned = apply_action(
            &mut state,
            &bundle,
            GameAction::LearnAbilityFromItem {
                learner_id: actor_id.clone(),
                item_id: scroll.instance_id.clone(),
                grant_id: source_grant_id,
            },
        );
        assert!(learned.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityGranted { owner_id, template_id, source: AbilityGrantSource::Studied, .. }
                if owner_id == &actor_id && template_id == "spectral_spark"
        )));
        assert!(owner_has_permanent_ability(
            &state,
            &owner,
            "spectral_spark"
        ));
        assert!(!owner_has_permanent_ability(
            &state,
            &ItemOwner::Player,
            "spectral_spark"
        ));
        assert!(state.entity_inventory(&owner).is_some_and(|inventory| {
            inventory
                .iter()
                .all(|item| item.instance_id != scroll.instance_id)
        }));
    }

    #[test]
    fn generic_progress_award_uses_only_authored_pools_for_the_controlled_player() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;

        let awarded = apply_action(
            &mut state,
            &bundle,
            GameAction::AwardProgress {
                recipient_id: "player".to_owned(),
                pool: "necromancy".to_owned(),
                amount: 3,
            },
        );
        assert!(matches!(
            awarded.events.as_slice(),
            [GameEvent::ProgressAwarded { origin_id, pool, amount: 3, total: 3 }]
                if origin_id == "player" && pool == "necromancy"
        ));
        assert_eq!(state.progress_pools.get("necromancy"), Some(&3));

        for (recipient_id, pool, amount) in [
            ("player", "invented_pool", 1),
            ("someone_else", "necromancy", 1),
            ("player", "necromancy", 0),
        ] {
            let rejected = apply_action(
                &mut state,
                &bundle,
                GameAction::AwardProgress {
                    recipient_id: recipient_id.to_owned(),
                    pool: pool.to_owned(),
                    amount,
                },
            );
            assert!(matches!(
                rejected.events.as_slice(),
                [GameEvent::ActionRejected { .. }]
            ));
        }
        assert_eq!(state.progress_pools.get("necromancy"), Some(&3));
    }

    #[test]
    fn generic_grant_action_attaches_a_finite_temporary_ability_to_an_exact_item() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let item_id = state.inventory[0].instance_id.clone();

        let granted = apply_action(
            &mut state,
            &bundle,
            GameAction::GrantAbility {
                owner: ItemOwner::Entity(item_id.clone()),
                template_id: "grave_bolt".to_owned(),
                remaining_uses: Some(1),
                duration_turns: Some(3),
                source: AbilityGrantSource::Temporary,
            },
        );

        let grant = state
            .ability_grants
            .iter()
            .find(|grant| grant.owner == ItemOwner::Entity(item_id.clone()))
            .expect("grant belongs to the exact item instance");
        assert_eq!(grant.remaining_uses, Some(1));
        assert_eq!(grant.expires_at_turn, Some(state.turn_index + 3));
        assert!(state.ability_grant_is_accessible_to(grant, "player"));
        assert!(matches!(
            granted.events.as_slice(),
            [GameEvent::AbilityGranted {
                owner_id,
                template_id,
                source: AbilityGrantSource::Temporary,
                ..
            }] if owner_id == &item_id && template_id == "grave_bolt"
        ));

        let mut final_turn = None;
        for _ in 0..3 {
            final_turn = Some(apply_action(&mut state, &bundle, GameAction::Look));
        }
        assert!(
            !state
                .ability_grants
                .iter()
                .any(|grant| grant.owner == ItemOwner::Entity(item_id.clone())),
            "an elapsed temporary grant must not remain authoritative state"
        );
        assert!(
            final_turn
                .expect("three accepted turns")
                .events
                .iter()
                .any(|event| matches!(
                    event,
                    GameEvent::AbilityGrantExpired { owner_id, template_id, .. }
                        if owner_id == &item_id && template_id == "grave_bolt"
                ))
        );
    }

    #[test]
    fn generic_grant_action_rejects_dead_finite_grants() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let grants_before = state.ability_grants.len();

        for (remaining_uses, duration_turns) in [(Some(0), None), (None, Some(0))] {
            let outcome = apply_action(
                &mut state,
                &bundle,
                GameAction::GrantAbility {
                    owner: ItemOwner::Player,
                    template_id: "grave_bolt".to_owned(),
                    remaining_uses,
                    duration_turns,
                    source: AbilityGrantSource::Temporary,
                },
            );
            assert!(matches!(
                outcome.events.as_slice(),
                [GameEvent::ActionRejected { .. }]
            ));
        }
        assert_eq!(
            state.ability_grants.len(),
            grants_before,
            "invalid grants must not enter authoritative state"
        );
    }

    #[test]
    fn healing_ability_uses_the_same_runtime_actor_health_path_for_an_npc() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle.abilities.push(AbilityTemplate {
            id: "mend_actor".to_owned(),
            name: "Mend Actor".to_owned(),
            description: "A test healing ability.".to_owned(),
            tags: Vec::new(),
            required_tags: Vec::new(),
            progress_pool: "support".to_owned(),
            required_xp: 0,
            xp_on_success: 0,
            learnable: true,
            scroll_learnable: true,
            scroll_learning_tags: Vec::new(),
            default_uses: None,
            duration_turns: None,
            cooldown_turns: 0,
            action_cost: 1,
            effect: Some(AbilityEffect::Healing {
                amount: 4,
                required_target_tags: Vec::new(),
            }),
        });
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "healing_npc".to_owned(),
                template_id: "healing_npc".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Healing NPC".to_owned(),
                description: "A wounded test actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    current_hp: Some(3),
                    max_hp: Some(10),
                    capabilities: vec!["health".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.ability_grants.push(AbilityGrant {
            instance_id: "npc_mend_grant".to_owned(),
            template_id: "mend_actor".to_owned(),
            owner: ItemOwner::Entity("healing_npc".to_owned()),
            remaining_uses: None,
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Innate,
        });

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "healing_npc".to_owned(),
                grant_id: "npc_mend_grant".to_owned(),
                target: "Healing NPC".to_owned(),
            },
        );

        assert_eq!(
            state.location_npcs[&state.current_location_id][0]
                .actor
                .current_hp,
            Some(7)
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::HealthRestored { origin_id, target_id, amount: 4, remaining_hp: 7 }
                if origin_id == "healing_npc" && target_id == "healing_npc"
        )));
    }

    #[test]
    fn ordinary_healing_requires_alive_but_revival_requires_dead_and_restores_an_incapacitated_player()
     {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle.abilities.push(AbilityTemplate {
            id: "ordinary_mend".to_owned(),
            name: "Ordinary Mend".to_owned(),
            description: "A living-target healing fixture.".to_owned(),
            tags: Vec::new(),
            required_tags: Vec::new(),
            progress_pool: "support".to_owned(),
            required_xp: 0,
            xp_on_success: 0,
            learnable: false,
            scroll_learnable: false,
            scroll_learning_tags: Vec::new(),
            default_uses: None,
            duration_turns: None,
            cooldown_turns: 0,
            action_cost: 1,
            effect: Some(AbilityEffect::Healing {
                amount: 4,
                required_target_tags: Vec::new(),
            }),
        });
        bundle.abilities.push(AbilityTemplate {
            id: "test_revive".to_owned(),
            name: "Test Revive".to_owned(),
            description: "A dead-target revival fixture.".to_owned(),
            tags: Vec::new(),
            required_tags: Vec::new(),
            progress_pool: "support".to_owned(),
            required_xp: 0,
            xp_on_success: 0,
            learnable: false,
            scroll_learnable: false,
            scroll_learning_tags: Vec::new(),
            default_uses: None,
            duration_turns: None,
            cooldown_turns: 0,
            action_cost: 1,
            effect: Some(AbilityEffect::Revive {
                amount: 3,
                required_target_tags: vec!["dead".to_owned()],
            }),
        });
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id,
            vec![NpcInstance {
                instance_id: "field_medic".to_owned(),
                template_id: "field_medic".to_owned(),
                location_id: state.current_location_id.clone(),
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Field Medic".to_owned(),
                description: "A revival fixture.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    current_hp: Some(10),
                    max_hp: Some(10),
                    capabilities: vec!["health".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.ability_grants.extend([
            AbilityGrant {
                instance_id: "ordinary_mend_grant".to_owned(),
                template_id: "ordinary_mend".to_owned(),
                owner: ItemOwner::Entity("field_medic".to_owned()),
                remaining_uses: None,
                expires_at_turn: None,
                ready_at_turn: 0,
                source: AbilityGrantSource::Innate,
            },
            AbilityGrant {
                instance_id: "test_revive_grant".to_owned(),
                template_id: "test_revive".to_owned(),
                owner: ItemOwner::Entity("field_medic".to_owned()),
                remaining_uses: None,
                expires_at_turn: None,
                ready_at_turn: 0,
                source: AbilityGrantSource::Innate,
            },
        ]);
        state.hp = 0;
        assert!(state.runtime_actor_has_tag("player", "dead"));
        assert!(!state.runtime_actor_has_tag("player", "alive"));

        let healing = handle_use_ability(
            &mut state,
            &bundle,
            "field_medic",
            "ordinary_mend_grant",
            "player",
        );
        assert!(matches!(
            healing.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("alive")
        ));
        assert_eq!(state.hp, 0);

        let revived = handle_use_ability(
            &mut state,
            &bundle,
            "field_medic",
            "test_revive_grant",
            "player",
        );
        assert_eq!(state.hp, 3);
        assert!(state.runtime_actor_has_tag("player", "alive"));
        assert!(!state.runtime_actor_has_tag("player", "dead"));
        assert!(revived.events.iter().any(|event| matches!(
            event,
            GameEvent::HealthRestored { origin_id, target_id, amount: 3, remaining_hp: 3 }
                if origin_id == "field_medic" && target_id == "player"
        )));
    }

    #[test]
    fn movement_ability_uses_the_canonical_move_reducer_and_its_own_action_cost() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let ability = bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter ability");
        ability.effect = Some(AbilityEffect::Move);
        ability.action_cost = 4;
        ability.cooldown_turns = 2;

        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        state.ability_grants[0].remaining_uses = Some(1);

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
        assert_eq!(state.turn_index, 4);
        assert_eq!(state.actor_ready_at("player"), 4);
        assert_eq!(state.ability_grants[0].remaining_uses, Some(0));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::Moved { from_location_id, to_location_id, .. }
                if from_location_id == "front_verandah" && to_location_id == "kitchen"
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityUsed { effect, target_id, .. }
                if effect == "movement" && target_id == "kitchen"
        )));
    }

    #[test]
    fn failed_movement_ability_does_not_spend_its_grant_or_time() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter ability")
            .effect = Some(AbilityEffect::Move);
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        state.ability_grants[0].remaining_uses = Some(1);

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: "laundry".to_owned(),
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::MovementBlocked { .. }]
        ));
        assert_eq!(state.current_location_id, "front_verandah");
        assert_eq!(state.turn_index, 0);
        assert_eq!(state.actor_ready_at("player"), 0);
        assert_eq!(state.ability_grants[0].remaining_uses, Some(1));
        assert_eq!(state.ability_grants[0].ready_at_turn, 0);
    }

    #[test]
    fn movement_ability_cannot_bypass_a_generic_movement_gate() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter ability")
            .effect = Some(AbilityEffect::Move);
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        state.ability_grants[0].remaining_uses = Some(1);
        state.status_effects.push(StatusEffectInstance {
            instance_id: "rooted".to_owned(),
            template_id: "rooted".to_owned(),
            owner: StatusOwner::Scene,
            owner_id: None,
            trigger: EffectTrigger::Movement,
            duration: EffectDuration::Run,
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
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: "kitchen".to_owned(),
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }] if reason.contains("rooted")
        ));
        assert_eq!(state.current_location_id, "front_verandah");
        assert_eq!(state.turn_index, 0);
        assert_eq!(state.ability_grants[0].remaining_uses, Some(1));
    }

    #[test]
    fn movement_ability_uses_the_same_actor_path_for_an_npc() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle.abilities.push(AbilityTemplate {
            id: "npc_step".to_owned(),
            name: "NPC Step".to_owned(),
            description: "A test movement ability.".to_owned(),
            tags: Vec::new(),
            required_tags: Vec::new(),
            progress_pool: "support".to_owned(),
            required_xp: 0,
            xp_on_success: 0,
            learnable: true,
            scroll_learnable: true,
            scroll_learning_tags: Vec::new(),
            default_uses: None,
            duration_turns: None,
            cooldown_turns: 0,
            action_cost: 3,
            effect: Some(AbilityEffect::Move),
        });
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "walking_npc".to_owned(),
                template_id: "walking_npc".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Walking NPC".to_owned(),
                description: "A test actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    can_move: true,
                    capabilities: vec!["movement".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.ability_grants.push(AbilityGrant {
            instance_id: "npc_step_grant".to_owned(),
            template_id: "npc_step".to_owned(),
            owner: ItemOwner::Entity("walking_npc".to_owned()),
            remaining_uses: Some(1),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Innate,
        });

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "walking_npc".to_owned(),
                grant_id: "npc_step_grant".to_owned(),
                target: "kitchen".to_owned(),
            },
        );

        assert_eq!(state.current_location_id, "front_verandah");
        assert_eq!(
            state
                .runtime_actor("walking_npc")
                .expect("moved actor")
                .location_id,
            "kitchen"
        );
        assert_eq!(state.turn_index, 3);
        assert_eq!(state.actor_ready_at("walking_npc"), 3);
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ActorMoved { origin_id, from_location_id, to_location_id, .. }
                if origin_id == "walking_npc"
                    && from_location_id == "front_verandah"
                    && to_location_id == "kitchen"
        )));
    }

    #[test]
    fn named_actor_cast_intent_resolves_only_its_real_grant() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle.abilities.push(AbilityTemplate {
            id: "actor_mend".to_owned(),
            name: "Actor Mend".to_owned(),
            description: "A test ability.".to_owned(),
            tags: Vec::new(),
            required_tags: Vec::new(),
            progress_pool: "support".to_owned(),
            required_xp: 0,
            xp_on_success: 2,
            learnable: true,
            scroll_learnable: true,
            scroll_learning_tags: Vec::new(),
            default_uses: None,
            duration_turns: None,
            cooldown_turns: 0,
            action_cost: 1,
            effect: Some(AbilityEffect::Healing {
                amount: 1,
                required_target_tags: Vec::new(),
            }),
        });
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "caster_npc".to_owned(),
                template_id: "caster_npc".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Caster NPC".to_owned(),
                description: "A test caster.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState {
                    current_hp: Some(5),
                    max_hp: Some(10),
                    capabilities: vec!["health".to_owned()],
                    ..Default::default()
                },
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.ability_grants.push(AbilityGrant {
            instance_id: "caster_grant".to_owned(),
            template_id: "actor_mend".to_owned(),
            owner: ItemOwner::Entity("caster_npc".to_owned()),
            remaining_uses: Some(1),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Innate,
        });

        let (origin_id, grant_id, target) =
            resolve_actor_ability_intent(&state, &bundle, "Caster NPC, cast Actor Mend on self")
                .expect("actor instruction")
                .expect("owned grant resolves");
        assert_eq!(origin_id, "caster_npc");
        assert_eq!(grant_id, "caster_grant");
        assert_eq!(target, "self");
        assert!(
            resolve_actor_ability_intent(
                &state,
                &bundle,
                "Caster NPC, cast imaginary spell on self"
            )
            .expect("actor instruction")
            .is_err()
        );
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id,
                grant_id,
                target,
            },
        );
        assert!(
            outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityUsed { .. }))
        );
        assert!(
            !outcome
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::ProgressAwarded { .. })),
            "a non-player actor must not mutate controlled-player progression"
        );
        assert!(state.progress_pools.is_empty());

        let unlock = apply_action(
            &mut state,
            &bundle,
            GameAction::UnlockAbility {
                owner: ItemOwner::Entity("caster_npc".to_owned()),
                template_id: "actor_mend".to_owned(),
            },
        );
        assert!(matches!(
            unlock.events.as_slice(),
            [GameEvent::ActionRejected { reason }]
                if reason.contains("controlled actor's progression ledger")
        ));
    }

    #[test]
    fn cooldown_uses_absolute_world_turns_and_only_successful_casts_start_it() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let ability = bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter ability");
        ability.cooldown_turns = 3;
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        let target_id = state.location_enemies[&state.current_location_id][0].clone();

        let failed = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: grant_id.clone(),
                target: "missing target".to_owned(),
            },
        );
        assert!(matches!(
            failed.events.as_slice(),
            [GameEvent::ActionRejected { .. }]
        ));
        assert_eq!(
            ability_grant_cooldown_remaining(
                &state,
                state
                    .ability_grants
                    .iter()
                    .find(|grant| grant.instance_id == "starter_ability_grave_bolt")
                    .expect("starter grant"),
            ),
            0
        );
        assert_eq!(
            state.turn_index, 0,
            "rejections do not advance the world clock"
        );

        let successful = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: grant_id.clone(),
                target: target_id.clone(),
            },
        );
        assert_eq!(state.ability_grants[0].ready_at_turn, 3);
        assert_eq!(state.turn_index, 1);
        assert!(successful.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityCooldownSet { grant_id: event_grant, ready_at_turn: 3, .. }
                if event_grant == &grant_id
        )));
        assert_eq!(
            ability_grant_cooldown_remaining(&state, &state.ability_grants[0]),
            2
        );

        let blocked = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: grant_id.clone(),
                target: target_id.clone(),
            },
        );
        assert!(
            matches!(blocked.events.as_slice(), [GameEvent::ActionRejected { reason }]
            if reason.contains("cooling down"))
        );
        assert_eq!(state.ability_grants[0].ready_at_turn, 3);

        state.turn_index = 3;
        assert_eq!(
            ability_grant_cooldown_remaining(&state, &state.ability_grants[0]),
            0
        );
        let ready = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: target_id,
            },
        );
        assert!(
            ready
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityUsed { .. }))
        );
    }

    #[test]
    fn cooldown_is_independent_per_grant_and_composes_with_finite_uses() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle
            .abilities
            .iter_mut()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter ability")
            .cooldown_turns = 2;
        let mut state = generate_new_run(&bundle).state;
        let first_id = state.ability_grants[0].instance_id.clone();
        state.ability_grants[0].remaining_uses = Some(2);
        state.ability_grants.push(AbilityGrant {
            instance_id: "second_grave_bolt".to_owned(),
            template_id: "grave_bolt".to_owned(),
            owner: ItemOwner::Player,
            remaining_uses: Some(1),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Item,
        });
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let first = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: first_id,
                target: target_id.clone(),
            },
        );
        assert!(
            first
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityUsed { .. }))
        );
        assert_eq!(state.ability_grants[0].ready_at_turn, 2);
        assert_eq!(state.ability_grants[1].ready_at_turn, 0);

        let second = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: "second_grave_bolt".to_owned(),
                target: target_id,
            },
        );
        assert!(second.events.iter().any(|event| matches!(
            event,
            GameEvent::AbilityUsed {
                remaining_uses: Some(0),
                ..
            }
        )));
        let first_grant = state
            .ability_grants
            .iter()
            .find(|grant| grant.instance_id == "starter_ability_grave_bolt")
            .expect("starter grant");
        let second_grant = state
            .ability_grants
            .iter()
            .find(|grant| grant.instance_id == "second_grave_bolt")
            .expect("independent grant");
        assert_eq!(first_grant.remaining_uses, Some(1));
        assert_eq!(second_grant.remaining_uses, Some(0));
        assert_eq!(first_grant.ready_at_turn, 2);
        assert_eq!(second_grant.ready_at_turn, 3);
    }

    #[test]
    fn zero_cooldown_reuses_normally_without_a_cooldown_receipt() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let first = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id: grant_id.clone(),
                target: target_id.clone(),
            },
        );
        let second = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: target_id,
            },
        );
        assert!(
            first
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityUsed { .. }))
        );
        assert!(
            second
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityUsed { .. }))
        );
        assert!(
            !first
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityCooldownSet { .. }))
        );
        assert!(
            !second
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AbilityCooldownSet { .. }))
        );
        assert_eq!(
            ability_grant_cooldown_remaining(
                &state,
                state
                    .ability_grants
                    .iter()
                    .find(|grant| grant.instance_id == "starter_ability_grave_bolt")
                    .expect("starter grant"),
            ),
            0
        );
    }

    #[test]
    fn status_ability_uses_the_shared_status_ledger_and_combat_evaluator() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut ward = bundle
            .abilities
            .iter()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter")
            .clone();
        ward.id = "grave_edge".to_owned();
        ward.name = "Grave Edge".to_owned();
        ward.effect = Some(AbilityEffect::Status {
            trigger: AbilityStatusTrigger::Combat,
            duration: AbilityStatusDuration::UntilUsed,
            additive_damage: 3,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
        });
        bundle.abilities.push(ward);
        let mut state = generate_new_run(&bundle).state;
        let grant = super::grant_ability(
            &mut state,
            &bundle,
            ItemOwner::Player,
            "grave_edge",
            None,
            None,
            AbilityGrantSource::Granted,
        );
        let grant_id = match grant.events.as_slice() {
            [GameEvent::AbilityGranted { grant_id, .. }] => grant_id.clone(),
            _ => panic!("expected grant"),
        };
        let cast = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: String::new(),
            },
        );
        assert!(cast.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectApplied {
                trigger: EffectTrigger::Combat,
                duration: EffectDuration::UntilUsed,
                ..
            }
        )));
        assert_eq!(state.status_effects.len(), 1);
        assert_eq!(state.status_effects[0].owner, StatusOwner::Entity);
        assert_eq!(state.status_effects[0].owner_id.as_deref(), Some("player"));
        let attack = apply_action(&mut state, &bundle, GameAction::Attack);
        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AttackResolved { damage: 4, .. }))
        );
        assert!(
            attack
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::StatusEffectExpired { .. }))
        );
    }

    #[test]
    fn entity_owned_combat_status_applies_to_its_npc_caster_only() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut edge = bundle
            .abilities
            .iter()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter")
            .clone();
        edge.id = "npc_edge".to_owned();
        edge.name = "NPC Edge".to_owned();
        edge.required_tags = Vec::new();
        edge.effect = Some(AbilityEffect::Status {
            trigger: AbilityStatusTrigger::Combat,
            duration: AbilityStatusDuration::UntilUsed,
            additive_damage: 3,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
        });
        bundle.abilities.push(edge);
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "edge_npc".to_owned(),
                template_id: "edge_npc".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Edge NPC".to_owned(),
                description: "A combat test actor.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState::configured(Some(10), Some(4), false, false, false, true),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        state.ability_grants.push(AbilityGrant {
            instance_id: "edge_npc_grant".to_owned(),
            template_id: "npc_edge".to_owned(),
            owner: ItemOwner::Entity("edge_npc".to_owned()),
            remaining_uses: Some(1),
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Innate,
        });
        let target_id = state.location_enemies[&state.current_location_id][0].clone();

        apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "edge_npc".to_owned(),
                grant_id: "edge_npc_grant".to_owned(),
                target: String::new(),
            },
        );
        let attack = apply_action(
            &mut state,
            &bundle,
            GameAction::ActorAttack {
                origin_id: "edge_npc".to_owned(),
                target: target_id,
            },
        );

        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { origin_id, damage: 7, .. } if origin_id == "edge_npc"
        )));
        assert!(attack.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id.starts_with("ability_status_edge_npc_grant")
        )));
    }

    #[test]
    fn damage_ability_uses_the_same_combat_status_contract() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let base_damage = match bundle
            .abilities
            .iter()
            .find(|ability| ability.id == "grave_bolt")
            .and_then(|ability| ability.effect.as_ref())
        {
            Some(AbilityEffect::Damage { amount }) => *amount,
            _ => panic!("starter ability must damage"),
        };
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        state.status_effects.push(StatusEffectInstance {
            instance_id: "caster_edge".to_owned(),
            template_id: "caster_edge".to_owned(),
            owner: StatusOwner::Entity,
            owner_id: Some("player".to_owned()),
            trigger: EffectTrigger::Combat,
            duration: EffectDuration::UntilUsed,
            additive_damage: 3,
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
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: target_id,
            },
        );

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { damage, .. } if *damage == base_damage + 3
        )));
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::StatusEffectExpired { instance_id } if instance_id == "caster_edge"
        )));
    }

    #[test]
    fn damage_ability_respects_an_encounters_health_contract() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let grant_id = state.ability_grants[0].instance_id.clone();
        let target_id = state.location_enemies[&state.current_location_id][0].clone();
        let hp_before = state.enemy_hp[&target_id];
        state
            .enemy_instances
            .get_mut(&target_id)
            .expect("fixture encounter")
            .capabilities = vec!["combat".to_owned()];

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: target_id.clone(),
            },
        );

        assert!(matches!(
            outcome.events.as_slice(),
            [GameEvent::ActionRejected { reason }]
                if reason == "No valid hostile target is present for that ability."
        ));
        assert_eq!(state.enemy_hp[&target_id], hp_before);
    }

    #[test]
    fn damage_ability_uses_the_shared_hostile_path_for_an_npc() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        let location_id = state.current_location_id.clone();
        state.location_npcs.insert(
            location_id.clone(),
            vec![NpcInstance {
                instance_id: "hostile_ability_target".to_owned(),
                template_id: "hostile_ability_target".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Hostile Ability Target".to_owned(),
                description: "A reducer fixture.".to_owned(),
                faction: String::new(),
                tags: vec!["fixture".to_owned()],
                disposition: ActorDisposition::Hostile,
                actor: ActorState::configured(Some(4), Some(1), false, false, false, true),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            }],
        );
        let grant_id = state.ability_grants[0].instance_id.clone();

        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: "Hostile Ability Target".to_owned(),
            },
        );

        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved { target_id, target_kind: EncounterKind::Npc, damage: 2, .. }
                if target_id == "hostile_ability_target"
        )));
        assert_eq!(
            state.location_npcs[&state.current_location_id][0]
                .actor
                .current_hp,
            Some(2)
        );
    }

    #[test]
    fn discovery_ability_uses_the_same_route_ledger_as_item_utility() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut ability = bundle
            .abilities
            .iter()
            .find(|ability| ability.id == "grave_bolt")
            .expect("starter")
            .clone();
        ability.id = "grave_sight".to_owned();
        ability.name = "Grave Sight".to_owned();
        ability.effect = Some(AbilityEffect::RevealConnections);
        bundle.abilities.push(ability);
        let mut state = generate_new_run(&bundle).state;
        let grant = super::grant_ability(
            &mut state,
            &bundle,
            ItemOwner::Player,
            "grave_sight",
            None,
            None,
            AbilityGrantSource::Granted,
        );
        let grant_id = match grant.events.as_slice() {
            [GameEvent::AbilityGranted { grant_id, .. }] => grant_id.clone(),
            _ => panic!("expected grant"),
        };
        let outcome = apply_action(
            &mut state,
            &bundle,
            GameAction::UseAbility {
                origin_id: "player".to_owned(),
                grant_id,
                target: String::new(),
            },
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::ConnectionsRevealed { origin_id, count, .. }
                if origin_id == "player" && *count > 0
        )));
        assert!(state.known_locations.contains("kitchen"));
    }

    #[test]
    fn progression_levels_award_configured_hp_without_changing_current_hp() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle.rules.hp_per_skill_level = 2;
        let mut second = bundle
            .abilities
            .iter()
            .find(|ability| ability.id == "bone_lance")
            .expect("ability")
            .clone();
        second.id = "grave_wall".to_owned();
        second.name = "Grave Wall".to_owned();
        second.required_xp = 5;
        bundle.abilities.push(second);
        let mut state = generate_new_run(&bundle).state;
        state.hp = 4;
        let outcome = award_progress(&mut state, &bundle, "player", "necromancy", 10);

        assert_eq!(state.progress_pools.get("necromancy"), Some(&10));
        assert_eq!(
            state.max_hp, 14,
            "two distinct milestones award two increments"
        );
        assert_eq!(state.hp, 4, "a level-up never silently heals current HP");
        assert_eq!(
            outcome
                .events
                .iter()
                .filter(|event| matches!(event, GameEvent::SkillLevelGained { .. }))
                .count(),
            2
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::MaxHpIncreased {
                amount: 4,
                max_hp: 14,
                ..
            }
        )));
        assert!(owner_has_unlock_choice_for_template(
            &state,
            &bundle,
            &ItemOwner::Player,
            "bone_lance"
        ));
    }

    #[test]
    fn controlled_actor_progression_increases_that_actors_maximum_health() {
        let mut bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        bundle.rules.hp_per_skill_level = 2;
        let mut milestone = bundle
            .abilities
            .iter()
            .find(|ability| ability.id == "bone_lance")
            .expect("fixture ability")
            .clone();
        milestone.id = "progression_fixture_milestone".to_owned();
        milestone.required_xp = 5;
        bundle.abilities.push(milestone);
        let mut state = generate_new_run(&bundle).state;
        let actor_id = "progression_fixture".to_owned();
        let location_id = state.current_location_id.clone();
        state
            .location_npcs
            .entry(location_id.clone())
            .or_default()
            .push(NpcInstance {
                instance_id: actor_id.clone(),
                template_id: "progression_fixture".to_owned(),
                location_id,
                lifecycle: InstanceLifecycle::Active,
                provenance: "test".to_owned(),
                name: "Progression fixture".to_owned(),
                description: "Uses the shared progression path.".to_owned(),
                faction: "test".to_owned(),
                tags: Vec::new(),
                disposition: ActorDisposition::Friendly,
                actor: ActorState::configured(Some(4), Some(1), false, false, false, false),
                quest_objective_ids: Vec::new(),
                inventory: Vec::new(),
                equipped_item_id: None,
            });
        state.player_entity_id = actor_id.clone();
        let legacy_maximum = state.max_hp;

        let outcome = award_progress(&mut state, &bundle, &actor_id, "necromancy", 5);

        assert_eq!(state.max_hp, legacy_maximum);
        assert!(state.progress_pools.is_empty());
        assert_eq!(
            state.progress_pools_for_actor(&actor_id).get("necromancy"),
            Some(&5),
            "a controller swap must not inherit the default player's XP ledger"
        );
        assert_eq!(
            state
                .runtime_actor(&actor_id)
                .and_then(|actor| actor.actor.max_hp),
            Some(6)
        );
        assert_eq!(
            state
                .runtime_actor(&actor_id)
                .and_then(|actor| actor.actor.current_hp),
            Some(4),
            "a progression reward must not silently heal the actor"
        );
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::MaxHpIncreased {
                origin_id,
                amount: 2,
                max_hp: 6,
            } if origin_id == &actor_id
        )));
    }

    #[test]
    fn progress_without_a_new_milestone_does_not_award_hp_twice() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic").expect("fixture");
        let mut state = generate_new_run(&bundle).state;
        state.progress_pools.insert("necromancy".to_owned(), 10);
        let max_hp = state.max_hp;
        let outcome = award_progress(&mut state, &bundle, "player", "necromancy", 1);
        assert_eq!(state.max_hp, max_hp);
        assert!(!outcome.events.iter().any(|event| matches!(
            event,
            GameEvent::SkillLevelGained { .. } | GameEvent::MaxHpIncreased { .. }
        )));
    }
}
