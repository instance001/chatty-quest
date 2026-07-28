#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::data::datapacks::DatapackBundle;
use crate::game::derived::run_phase_label;
use crate::game::{GameEvent, RunState};

const HANDOFF_PROTOCOL_VERSION: &str = "chatty_quest_handoff_v0";
const RUN_SNAPSHOT_PAYLOAD_VERSION: &str = "run_snapshot_v0";
const RECENT_EVENT_LIMIT: usize = 12;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandoffEnvelope {
    pub protocol_version: String,
    pub packet_id: String,
    pub packet_kind: String,
    pub source_module: String,
    pub destination_kind: String,
    pub created_at: String,
    pub scenario_id: String,
    pub run_id: String,
    pub payload_version: String,
    pub tags: Vec<String>,
    pub summary: String,
    pub body: RunSnapshotPayload,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunSnapshotPayload {
    pub runtime_schema_version: String,
    pub datapack_id: String,
    pub datapack_display_name: String,
    pub current_location: SnapshotLocation,
    pub player_state: SnapshotPlayerState,
    pub objective_state: SnapshotObjectiveState,
    pub location_state: SnapshotLocationState,
    pub encounter_state: SnapshotEncounterState,
    pub inventory_state: SnapshotInventoryState,
    pub recent_events: Vec<GameEvent>,
    pub rolling_summary: Vec<String>,
    pub important_flags: Vec<String>,
    pub boundary_note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotLocation {
    pub id: String,
    pub name: String,
    pub known_connected_exits: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotPlayerState {
    pub hp: i32,
    pub max_hp: i32,
    pub noise_level: i32,
    pub run_phase: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotObjectiveState {
    pub id: String,
    pub name: String,
    pub completed: bool,
    pub required_item_id: Option<String>,
    pub required_location_id: Option<String>,
    pub target_boss_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotLocationState {
    pub known_location_count: usize,
    pub visited_location_count: usize,
    pub locked_locations: Vec<String>,
    pub barricaded_locations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotEncounterState {
    pub local_live_enemies: Vec<String>,
    pub local_live_bosses: Vec<String>,
    pub defeated_enemies: Vec<String>,
    pub defeated_bosses: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotInventoryState {
    pub carried_items: Vec<String>,
    pub equipped_item_id: Option<String>,
}

pub fn build_run_snapshot_envelope(
    bundle: &DatapackBundle,
    run: &RunState,
    recent_events: &[GameEvent],
) -> HandoffEnvelope {
    let run_phase = run_phase_label(run).to_owned();
    let current_location = bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id);
    let current_location_name = current_location
        .map(|location| location.name.clone())
        .unwrap_or_else(|| run.current_location_id.clone());
    let known_connected_exits = current_location
        .map(|location| {
            let mut exits = location
                .connections
                .iter()
                .filter(|location_id| run.known_locations.contains(*location_id))
                .cloned()
                .collect::<Vec<_>>();
            exits.sort();
            exits
        })
        .unwrap_or_default();

    let mut tags = vec![
        "dry_contract".to_owned(),
        "run_snapshot".to_owned(),
        run_phase.to_ascii_lowercase(),
    ];
    if run.active_objective.completed {
        tags.push("objective_complete".to_owned());
    }
    tags.sort();
    tags.dedup();

    let summary = format!(
        "Run snapshot for {} at {} ({})",
        run.datapack_display_name, current_location_name, run_phase
    );

    HandoffEnvelope {
        protocol_version: HANDOFF_PROTOCOL_VERSION.to_owned(),
        packet_id: format!(
            "dry-run-snapshot-{}-{}",
            run.datapack_id, run.current_location_id
        ),
        packet_kind: "run_snapshot".to_owned(),
        source_module: "chatty_quest".to_owned(),
        destination_kind: "local_archive".to_owned(),
        created_at: "dry_contract_no_runtime_export".to_owned(),
        scenario_id: run.datapack_id.clone(),
        run_id: format!("local-run-{}", run.datapack_id),
        payload_version: RUN_SNAPSHOT_PAYLOAD_VERSION.to_owned(),
        tags,
        summary,
        body: RunSnapshotPayload {
            runtime_schema_version: "run_state_v1".to_owned(),
            datapack_id: run.datapack_id.clone(),
            datapack_display_name: run.datapack_display_name.clone(),
            current_location: SnapshotLocation {
                id: run.current_location_id.clone(),
                name: current_location_name,
                known_connected_exits,
            },
            player_state: SnapshotPlayerState {
                hp: run.hp,
                max_hp: run.max_hp,
                noise_level: run.noise_level,
                run_phase,
            },
            objective_state: SnapshotObjectiveState {
                id: run.active_objective.id.clone(),
                name: run.active_objective.name.clone(),
                completed: run.active_objective.completed,
                required_item_id: run.active_objective.required_item_id.clone(),
                required_location_id: run.active_objective.required_location_id.clone(),
                target_boss_id: run.active_objective.target_boss_id.clone(),
            },
            location_state: SnapshotLocationState {
                known_location_count: run.known_locations.len(),
                visited_location_count: run.visited_locations.len(),
                locked_locations: sorted_strings(&run.locked_locations),
                barricaded_locations: sorted_strings(&run.barricaded_locations),
            },
            encounter_state: SnapshotEncounterState {
                local_live_enemies: local_live_ids(
                    &run.location_enemies,
                    &run.enemies_alive,
                    &run.current_location_id,
                ),
                local_live_bosses: local_live_ids(
                    &run.location_bosses,
                    &run.bosses_alive,
                    &run.current_location_id,
                ),
                defeated_enemies: sorted_strings(&run.enemies_defeated),
                defeated_bosses: sorted_strings(&run.bosses_defeated),
            },
            inventory_state: SnapshotInventoryState {
                carried_items: run.inventory.iter().map(|item| item.id.clone()).collect(),
                equipped_item_id: run.equipped_item_id.clone(),
            },
            recent_events: recent_events
                .iter()
                .rev()
                .take(RECENT_EVENT_LIMIT)
                .cloned()
                .collect(),
            rolling_summary: run.rolling_summary.clone(),
            important_flags: important_flags(run),
            boundary_note: run.boundary_response.clone(),
        },
    }
}

fn sorted_strings(values: &std::collections::HashSet<String>) -> Vec<String> {
    let mut sorted = values.iter().cloned().collect::<Vec<_>>();
    sorted.sort();
    sorted
}

fn local_live_ids(
    placements: &std::collections::HashMap<String, Vec<String>>,
    live_ids: &std::collections::HashSet<String>,
    location_id: &str,
) -> Vec<String> {
    let mut ids = placements
        .get(location_id)
        .into_iter()
        .flatten()
        .filter(|id| live_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn important_flags(run: &RunState) -> Vec<String> {
    let mut flags = Vec::new();

    if run.hp <= 0 {
        flags.push("run_lost".to_owned());
    }
    if run.active_objective.completed {
        flags.push("objective_complete".to_owned());
        flags.push("epilogue_open".to_owned());
    }
    if !run.locked_locations.is_empty() {
        flags.push("locked_routes_present".to_owned());
    }
    if !run.barricaded_locations.is_empty() {
        flags.push("barricades_present".to_owned());
    }
    if run.noise_level >= 2 {
        flags.push("high_noise".to_owned());
    }

    flags
}

#[cfg(test)]
mod tests {
    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::{GameAction, GameEvent, apply_action, generate_new_run};

    use super::build_run_snapshot_envelope;

    #[test]
    fn run_snapshot_envelope_packages_bounded_copy_of_truth() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut run = generate_new_run(&bundle).state;
        let moved = apply_action(
            &mut run,
            &bundle,
            GameAction::Move {
                destination: "kitchen".to_owned(),
            },
        );

        let envelope = build_run_snapshot_envelope(&bundle, &run, &moved.events);

        assert_eq!(envelope.protocol_version, "chatty_quest_handoff_v0");
        assert_eq!(envelope.packet_kind, "run_snapshot");
        assert_eq!(envelope.source_module, "chatty_quest");
        assert_eq!(envelope.destination_kind, "local_archive");
        assert_eq!(envelope.scenario_id, "property_siege_classic");
        assert_eq!(envelope.body.current_location.id, "kitchen");
        assert_eq!(envelope.body.player_state.run_phase, "Active");
        assert_eq!(envelope.body.objective_state.name, "Secure The Property");
        assert!(
            envelope
                .body
                .recent_events
                .iter()
                .any(|event| matches!(event, GameEvent::Moved { .. }))
        );
        assert!(
            envelope
                .body
                .rolling_summary
                .iter()
                .any(|line| line.contains("Moved from 'front_verandah' to 'kitchen'"))
        );
    }

    #[test]
    fn run_snapshot_marks_epilogue_without_transferring_authority() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut run = generate_new_run(&bundle).state;
        run.active_objective.completed = true;
        run.barricaded_locations.insert("front_verandah".to_owned());
        run.noise_level = 3;

        let envelope = build_run_snapshot_envelope(&bundle, &run, &[]);

        assert_eq!(envelope.body.player_state.run_phase, "Epilogue");
        assert!(
            envelope
                .body
                .important_flags
                .iter()
                .any(|flag| flag == "objective_complete")
        );
        assert!(
            envelope
                .body
                .important_flags
                .iter()
                .any(|flag| flag == "epilogue_open")
        );
        assert!(
            envelope
                .body
                .location_state
                .barricaded_locations
                .iter()
                .any(|location_id| location_id == "front_verandah")
        );
    }
}
