use std::fs;

use serde::{Deserialize, Serialize};

use crate::app_paths;
use crate::game::{GameEvent, RunState};

const SAVE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavePayload {
    #[serde(default = "default_save_version")]
    pub save_version: u32,
    pub selected_datapack: String,
    pub difficulty: f32,
    pub chaos_mode: f32,
    #[serde(default = "default_fog_mode")]
    pub fog_mode: String,
    pub dm_capsule: String,
    pub cpu_helper_model: String,
    pub gpu_narrator_model: String,
    pub active_tab: String,
    pub log_lines: Vec<String>,
    #[serde(default)]
    pub diagnostic_events: Vec<GameEvent>,
    pub run_state: RunState,
}

pub fn save_game(payload: &SavePayload) -> Result<String, String> {
    let save_path = app_paths::current_save_path();
    if let Some(parent) = save_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create save folder: {}.", err))?;
    }

    let json = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("Could not serialize save payload: {}.", err))?;
    fs::write(&save_path, json).map_err(|err| format!("Could not write save file: {}.", err))?;

    Ok(save_path.display().to_string())
}

pub fn load_game() -> Result<SavePayload, String> {
    let save_path = app_paths::current_save_path();
    let json = fs::read_to_string(&save_path)
        .map_err(|err| format!("Could not read save file: {}.", err))?;
    serde_json::from_str(&json).map_err(|err| format!("Could not parse save file: {}.", err))
}

pub fn current_save_path() -> String {
    app_paths::current_save_path().display().to_string()
}

pub fn current_save_version() -> u32 {
    SAVE_VERSION
}

fn default_save_version() -> u32 {
    SAVE_VERSION
}

fn default_fog_mode() -> String {
    "Known".to_owned()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::generate_new_run;

    use super::{SavePayload, current_save_path, current_save_version, load_game, save_game};

    struct SaveFileGuard {
        original_contents: Option<String>,
    }

    impl SaveFileGuard {
        fn capture() -> Self {
            let original_contents = fs::read_to_string(current_save_path()).ok();
            Self { original_contents }
        }
    }

    impl Drop for SaveFileGuard {
        fn drop(&mut self) {
            let save_path = current_save_path();
            let path = Path::new(&save_path);
            match &self.original_contents {
                Some(contents) => {
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(path, contents);
                }
                None => {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    #[test]
    fn save_and_load_roundtrip_preserves_core_run_state() {
        let _guard = SaveFileGuard::capture();
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let mut run = generate_new_run(&bundle).state;
        run.current_location_id = "garage".to_owned();
        run.hp = 7;
        run.active_objective.completed = true;
        run.locked_locations.remove("garage");
        run.broken_locked_locations.insert("back_garden".to_owned());
        run.barricaded_locations.insert("front_verandah".to_owned());
        run.noise_level = 2;
        run.noise_spawn_count = 1;
        run.spawned_enemy_targets.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "kitchen".to_owned(),
        );
        run.spawned_enemy_origins.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "back_garden".to_owned(),
        );
        run.spawned_enemy_searching
            .insert("noise_spawn_1_shambler_front_gate".to_owned());
        run.spawned_enemy_sight_targets.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "front_verandah".to_owned(),
        );
        run.spawned_enemy_sight_subjects.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "player".to_owned(),
        );
        run.spawned_enemy_sight_delays
            .insert("noise_spawn_1_shambler_front_gate".to_owned(), 1);

        let payload = SavePayload {
            save_version: current_save_version(),
            selected_datapack: "Property Siege Classic".to_owned(),
            difficulty: 0.35,
            chaos_mode: 0.10,
            fog_mode: "Known".to_owned(),
            dm_capsule: "Grim Survival".to_owned(),
            cpu_helper_model: "cpu-test".to_owned(),
            gpu_narrator_model: "gpu-test".to_owned(),
            active_tab: "Game".to_owned(),
            log_lines: vec!["System: test save".to_owned()],
            diagnostic_events: Vec::new(),
            run_state: run.clone(),
        };

        let saved_path = save_game(&payload).expect("expected save to succeed");
        assert!(Path::new(&saved_path).exists());

        let restored = load_game().expect("expected load to succeed");
        assert_eq!(restored.save_version, current_save_version());
        assert_eq!(restored.selected_datapack, payload.selected_datapack);
        assert_eq!(restored.run_state.current_location_id, "garage");
        assert_eq!(restored.run_state.hp, 7);
        assert!(restored.run_state.active_objective.completed);
        assert_eq!(
            restored
                .run_state
                .active_objective
                .target_boss_id
                .as_deref(),
            Some("brute_in_garage")
        );
        assert_eq!(
            restored
                .run_state
                .active_objective
                .required_item_id
                .as_deref(),
            Some("house_keys")
        );
        assert_eq!(
            restored
                .run_state
                .active_objective
                .required_location_id
                .as_deref(),
            Some("garage")
        );
        assert!(!restored.run_state.locked_locations.contains("garage"));
        assert!(
            restored
                .run_state
                .broken_locked_locations
                .contains("back_garden")
        );
        assert!(
            restored
                .run_state
                .barricaded_locations
                .contains("front_verandah")
        );
        assert_eq!(restored.run_state.noise_level, 2);
        assert_eq!(restored.run_state.noise_spawn_count, 1);
        assert_eq!(
            restored
                .run_state
                .spawned_enemy_targets
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("kitchen")
        );
        assert_eq!(
            restored
                .run_state
                .spawned_enemy_origins
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("back_garden")
        );
        assert!(
            restored
                .run_state
                .spawned_enemy_searching
                .contains("noise_spawn_1_shambler_front_gate")
        );
        assert_eq!(
            restored
                .run_state
                .spawned_enemy_sight_targets
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("front_verandah")
        );
        assert_eq!(
            restored
                .run_state
                .spawned_enemy_sight_subjects
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("player")
        );
        assert_eq!(
            restored
                .run_state
                .spawned_enemy_sight_delays
                .get("noise_spawn_1_shambler_front_gate")
                .copied(),
            Some(1)
        );
        assert_eq!(restored.run_state.inventory.len(), run.inventory.len());
        assert_eq!(restored.run_state.equipped_item_id, run.equipped_item_id);
    }
}
