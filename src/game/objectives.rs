use super::state::RunState;

pub(super) fn update_objective_completion(state: &mut RunState) -> bool {
    let boss_condition_met = state
        .active_objective
        .target_boss_id
        .as_ref()
        .is_none_or(|boss_id| state.bosses_defeated.contains(boss_id));
    let item_condition_met = state
        .active_objective
        .required_item_id
        .as_ref()
        .is_none_or(|item_id| state.inventory.iter().any(|item| &item.id == item_id));
    let location_condition_met = state
        .active_objective
        .required_location_id
        .as_ref()
        .is_none_or(|location_id| &state.current_location_id == location_id);
    let has_any_condition = state.active_objective.target_boss_id.is_some()
        || state.active_objective.required_item_id.is_some()
        || state.active_objective.required_location_id.is_some();
    let completed_now =
        has_any_condition && boss_condition_met && item_condition_met && location_condition_met;
    let just_completed = completed_now && !state.active_objective.completed;
    state.active_objective.completed = completed_now;
    just_completed
}

#[derive(Clone, Eq, PartialEq)]
pub(super) struct ObjectiveConditionStatus {
    label: String,
    met: bool,
}

pub(super) fn objective_condition_statuses(state: &RunState) -> Vec<ObjectiveConditionStatus> {
    let mut statuses = Vec::new();

    if let Some(required_item_id) = &state.active_objective.required_item_id {
        statuses.push(ObjectiveConditionStatus {
            label: format!("Hold item '{}'", required_item_id),
            met: state
                .inventory
                .iter()
                .any(|item| &item.id == required_item_id),
        });
    }

    if let Some(target_boss_id) = &state.active_objective.target_boss_id {
        statuses.push(ObjectiveConditionStatus {
            label: format!("Defeat boss '{}'", target_boss_id),
            met: state.bosses_defeated.contains(target_boss_id),
        });
    }

    if let Some(required_location_id) = &state.active_objective.required_location_id {
        statuses.push(ObjectiveConditionStatus {
            label: format!("Reach location '{}'", required_location_id),
            met: state.current_location_id == *required_location_id,
        });
    }

    statuses
}

pub(super) fn objective_progress_lines(
    before: &[ObjectiveConditionStatus],
    after: &[ObjectiveConditionStatus],
) -> Vec<String> {
    after
        .iter()
        .filter_map(|after_status| {
            let before_status = before
                .iter()
                .find(|status| status.label == after_status.label)?;
            if before_status.met == after_status.met {
                return None;
            }

            Some(format!(
                "Objective progress: {} is now {}.",
                after_status.label,
                if after_status.met {
                    "complete"
                } else {
                    "incomplete"
                }
            ))
        })
        .collect()
}
