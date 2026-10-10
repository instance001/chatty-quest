use super::state::{
    ActionTimingKind, EffectDuration, EffectTrigger, InstanceLifecycle, RunState,
    StatusEffectInstance, StatusOwner,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EffectEvaluation {
    pub additive_damage: i32,
    pub additive_healing: i32,
    pub expired_instance_ids: Vec<String>,
}

/// Read-only timing preview used before action dispatch. The reducer consumes listed
/// `until_used` effects only after a successful commit, so a denied or otherwise invalid action
/// cannot spend a timing status.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TimingEffectEvaluation {
    pub cost_delta: i64,
    pub blocked_by_instance_id: Option<String>,
    pub consume_on_success: Vec<String>,
}

pub fn evaluate_timing_effects(
    state: &RunState,
    origin_id: &str,
    action_kinds: &[ActionTimingKind],
    context_tags: &[String],
) -> TimingEffectEvaluation {
    let mut result = TimingEffectEvaluation::default();
    for effect in state.status_effects.iter().filter(|effect| {
        effect.lifecycle == InstanceLifecycle::Active
            && status_applies_to_actor(state, effect, origin_id)
            && (effect
                .timing_action_kinds
                .iter()
                .any(|kind| action_kinds.contains(kind))
                || effect
                    .blocked_action_kinds
                    .iter()
                    .any(|kind| action_kinds.contains(kind)))
            && tags_match(&effect.required_context_tags, context_tags)
    }) {
        if effect
            .blocked_action_kinds
            .iter()
            .any(|kind| action_kinds.contains(kind))
        {
            result.blocked_by_instance_id = Some(effect.instance_id.clone());
            return result;
        }
        if effect
            .timing_action_kinds
            .iter()
            .any(|kind| action_kinds.contains(kind))
        {
            result.cost_delta = result.cost_delta.saturating_add(effect.timing_cost_delta);
            if effect.duration == EffectDuration::UntilUsed {
                result.consume_on_success.push(effect.instance_id.clone());
            }
        }
    }
    result
}

pub fn consume_timing_effects(state: &mut RunState, instance_ids: &[String]) -> Vec<String> {
    let mut consumed = Vec::new();
    for effect in state.status_effects.iter_mut().filter(|effect| {
        effect.lifecycle == InstanceLifecycle::Active
            && instance_ids.iter().any(|id| id == &effect.instance_id)
    }) {
        effect.lifecycle = InstanceLifecycle::Consumed;
        consumed.push(effect.instance_id.clone());
    }
    consumed
}

pub fn adjusted_timing_cost(base_cost: u64, delta: i64) -> u64 {
    if delta.is_negative() {
        base_cost.saturating_sub(delta.unsigned_abs()).max(1)
    } else {
        base_cost.saturating_add(delta as u64).max(1)
    }
}

pub fn evaluate_status_effects(
    state: &mut RunState,
    origin_id: &str,
    trigger: EffectTrigger,
    target_tags: &[String],
    context_tags: &[String],
) -> EffectEvaluation {
    let location_id = state.current_location_id.clone();
    let mut result = EffectEvaluation::default();
    for effect in state.status_effects.iter_mut().filter(|effect| {
        effect.lifecycle == InstanceLifecycle::Active
            && effect.trigger == trigger
            && status_applies_to_scope(effect, &location_id, origin_id)
    }) {
        if !tags_match(&effect.required_target_tags, target_tags)
            || !tags_match(&effect.required_context_tags, context_tags)
        {
            continue;
        }
        result.additive_damage += effect.additive_damage;
        result.additive_healing += effect.additive_healing;
        if effect.duration == EffectDuration::UntilUsed {
            effect.lifecycle = InstanceLifecycle::Consumed;
            result.expired_instance_ids.push(effect.instance_id.clone());
        }
    }
    result
}

/// Status ownership is gameplay scope, never display metadata. The legacy `Player` marker names
/// the literal default player entity; new actor statuses use `Entity` with an exact runtime id.
/// Scene statuses are global to the active scene unless an authored location id narrows them.
pub fn status_applies_to_actor(
    state: &RunState,
    effect: &StatusEffectInstance,
    origin_id: &str,
) -> bool {
    status_applies_to_scope(effect, &state.current_location_id, origin_id)
}

fn status_applies_to_scope(
    effect: &StatusEffectInstance,
    location_id: &str,
    origin_id: &str,
) -> bool {
    match effect.owner {
        StatusOwner::Player => origin_id == "player",
        StatusOwner::Entity => effect.owner_id.as_deref() == Some(origin_id),
        StatusOwner::Scene => effect
            .owner_id
            .as_deref()
            .is_none_or(|effect_location_id| effect_location_id == location_id),
    }
}

fn tags_match(required: &[String], actual: &[String]) -> bool {
    required.iter().all(|required_tag| {
        actual
            .iter()
            .any(|actual_tag| actual_tag.eq_ignore_ascii_case(required_tag))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::state::{
        EffectDuration, EffectTrigger, InstanceLifecycle, StatusEffectInstance, StatusOwner,
    };

    #[test]
    fn evaluator_applies_matching_effect_and_expires_until_used() {
        let mut state = crate::game::generation::generate_new_run(
            &crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle"),
        )
        .state;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "effect_1".to_owned(),
            template_id: "giant_slayer".to_owned(),
            owner: StatusOwner::Player,
            owner_id: None,
            trigger: EffectTrigger::Combat,
            duration: EffectDuration::UntilUsed,
            additive_damage: 10,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: vec!["giant".to_owned()],
            required_context_tags: vec!["mountain_dew".to_owned()],
            lifecycle: InstanceLifecycle::Active,
        });
        let result = evaluate_status_effects(
            &mut state,
            "player",
            EffectTrigger::Combat,
            &["giant".to_owned()],
            &["mountain_dew".to_owned()],
        );
        assert_eq!(result.additive_damage, 10);
        assert_eq!(result.expired_instance_ids, vec!["effect_1"]);
        assert_eq!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        );
    }

    #[test]
    fn legacy_player_status_does_not_follow_a_non_player_controller() {
        let mut state = crate::game::generation::generate_new_run(
            &crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle"),
        )
        .state;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "legacy_player_edge".to_owned(),
            template_id: "legacy_player_edge".to_owned(),
            owner: StatusOwner::Player,
            owner_id: None,
            trigger: EffectTrigger::Combat,
            duration: EffectDuration::Run,
            additive_damage: 3,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });
        state.player_entity_id = "npc_controller".to_owned();

        let result = evaluate_status_effects(
            &mut state,
            "npc_controller",
            EffectTrigger::Combat,
            &[],
            &[],
        );

        assert_eq!(result.additive_damage, 0);
        assert_eq!(state.status_effects[0].lifecycle, InstanceLifecycle::Active);
    }

    #[test]
    fn entity_owned_status_does_not_leak_to_another_actor() {
        let mut state = crate::game::generation::generate_new_run(
            &crate::data::datapacks::load_datapack_bundle_by_folder("property_siege_classic")
                .expect("fixture bundle"),
        )
        .state;
        state.status_effects.push(StatusEffectInstance {
            instance_id: "npc_edge".to_owned(),
            template_id: "npc_edge".to_owned(),
            owner: StatusOwner::Entity,
            owner_id: Some("npc_1".to_owned()),
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

        let other = evaluate_status_effects(&mut state, "player", EffectTrigger::Combat, &[], &[]);
        assert_eq!(other.additive_damage, 0);
        assert_eq!(state.status_effects[0].lifecycle, InstanceLifecycle::Active);

        let owner = evaluate_status_effects(&mut state, "npc_1", EffectTrigger::Combat, &[], &[]);
        assert_eq!(owner.additive_damage, 3);
        assert_eq!(
            state.status_effects[0].lifecycle,
            InstanceLifecycle::Consumed
        );
    }
}
