use super::actions::{ActionOutcome, GameEvent, ItemUseEffect};
use super::state::RunState;

pub(super) const ROLLING_SUMMARY_LIMIT: usize = 24;

pub(super) fn append_rolling_summary(state: &mut RunState, outcome: &ActionOutcome) {
    let summary_lines = rolling_summary_lines(&outcome.events, &outcome.lines);
    state.rolling_summary.extend(summary_lines);
    trim_rolling_summary(&mut state.rolling_summary);
}

fn rolling_summary_lines(events: &[GameEvent], fallback_lines: &[String]) -> Vec<String> {
    let mut lines = events
        .iter()
        .filter_map(summarize_event)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        lines.extend(fallback_lines.iter().cloned());
    }

    lines
}

fn trim_rolling_summary(summary: &mut Vec<String>) {
    if summary.len() > ROLLING_SUMMARY_LIMIT {
        let remove_count = summary.len() - ROLLING_SUMMARY_LIMIT;
        summary.drain(0..remove_count);
    }
}

fn summarize_event(event: &GameEvent) -> Option<String> {
    match event {
        GameEvent::HelpShown => Some("Help was shown.".to_owned()),
        GameEvent::ActionRejected { reason } => Some(format!("Action rejected: {}", reason)),
        GameEvent::SelectedPackFailedValidation {
            folder_name,
            reason,
        } => Some(format!(
            "Selected cartridge '{}' failed validation: {}.",
            folder_name, reason
        )),
        GameEvent::LocationLooked { location_id } => {
            Some(format!("Looked around location '{}'.", location_id))
        }
        GameEvent::Moved {
            from_location_id,
            to_location_id,
        } => Some(format!(
            "Moved from '{}' to '{}'.",
            from_location_id, to_location_id
        )),
        GameEvent::MovementBlocked {
            attempted_destination,
        } => Some(format!(
            "Movement toward '{}' was blocked.",
            attempted_destination
        )),
        GameEvent::LocationUnlocked {
            location_id,
            item_id,
        } => Some(format!(
            "Unlocked location '{}' with item '{}'.",
            location_id, item_id
        )),
        GameEvent::LocationBarricaded {
            location_id,
            item_id,
        } => Some(format!(
            "Barricaded location '{}' with item '{}'.",
            location_id, item_id
        )),
        GameEvent::Inspected { target } => Some(format!("Inspected '{}'.", target)),
        GameEvent::ItemTaken { item_id } => Some(format!("Took item '{}'.", item_id)),
        GameEvent::ItemEquipped { item_id } => Some(format!("Equipped item '{}'.", item_id)),
        GameEvent::ItemUsed { item_id, effect } => match effect {
            ItemUseEffect::Healing { amount } => Some(format!(
                "Used item '{}' for healing {} HP.",
                item_id, amount
            )),
            ItemUseEffect::RevealedLocations { count } => Some(format!(
                "Used item '{}' to reveal {} connected location(s).",
                item_id, count
            )),
            ItemUseEffect::NoEffect => Some(format!("Used item '{}' with no effect.", item_id)),
        },
        GameEvent::AttackResolved {
            target_id,
            target_kind,
            damage,
            defeated,
        } => Some(format!(
            "Attack hit {:?} '{}' for {} damage{}.",
            target_kind,
            target_id,
            damage,
            if *defeated { " and defeated it" } else { "" }
        )),
        GameEvent::DamageTaken {
            amount,
            remaining_hp,
        } => Some(format!(
            "Took {} damage and dropped to {} HP.",
            amount, remaining_hp
        )),
        GameEvent::NoiseSpawnedEnemy {
            enemy_id,
            template_id,
            location_id,
        } => Some(format!(
            "Noise spawned enemy '{}' from template '{}' at '{}'.",
            enemy_id, template_id, location_id
        )),
        GameEvent::NoiseAttractorShifted {
            location_id,
            enemy_ids,
        } => Some(format!(
            "Noise attractor shifted to '{}' for spawned enemies: {}.",
            location_id,
            enemy_ids.join(", ")
        )),
        GameEvent::SightAttractorAcquired {
            enemy_id,
            subject_id,
            location_id,
        } => Some(format!(
            "Spawned enemy '{}' sighted '{}' at '{}'.",
            enemy_id, subject_id, location_id
        )),
        GameEvent::SightAttractorMissed {
            enemy_id,
            subject_id,
            location_id,
            detect_chance_percent,
            roll_percent,
        } => Some(format!(
            "Spawned enemy '{}' missed sighting '{}' at '{}' with {}% detect chance and roll {}.",
            enemy_id, subject_id, location_id, detect_chance_percent, roll_percent
        )),
        GameEvent::SightAttractorLost {
            enemy_id,
            subject_id,
        } => Some(format!(
            "Spawned enemy '{}' lost sight of '{}'.",
            enemy_id, subject_id
        )),
        GameEvent::SpawnedEnemyMoved {
            enemy_id,
            from_location_id,
            to_location_id,
            target_location_id,
        } => Some(format!(
            "Spawned enemy '{}' moved from '{}' to '{}' toward '{}'.",
            enemy_id, from_location_id, to_location_id, target_location_id
        )),
        GameEvent::SpawnedEnemyWaited {
            enemy_id,
            location_id,
            reason,
        } => Some(format!(
            "Spawned enemy '{}' waited at '{}': {}.",
            enemy_id, location_id, reason
        )),
        GameEvent::SpawnedEnemyAttackedHazard {
            enemy_id,
            hazard_kind,
            location_id,
            break_chance_percent,
            roll_percent,
            broken,
        } => Some(format!(
            "Spawned enemy '{}' attacked {:?} at '{}' with {}% break chance and rolled {}: broken={}.",
            enemy_id, hazard_kind, location_id, break_chance_percent, roll_percent, broken
        )),
        GameEvent::AttackWhiff => Some("Attack missed or found no target.".to_owned()),
        GameEvent::Waited { location_id } => Some(format!("Waited at '{}'.", location_id)),
        GameEvent::ObjectiveCompleted { objective_id } => {
            Some(format!("Completed objective '{}'.", objective_id))
        }
        GameEvent::RunWon => Some("Run won.".to_owned()),
        GameEvent::RunLost => Some("Run lost.".to_owned()),
    }
}
