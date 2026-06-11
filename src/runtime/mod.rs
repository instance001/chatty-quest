use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::game::{GameEvent, RunState};

const SAVE_PATH: &str = "runtime/saves/current_run.json";
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
    if let Some(parent) = Path::new(SAVE_PATH).parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Could not create save folder: {}.", err))?;
    }

    let json = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("Could not serialize save payload: {}.", err))?;
    fs::write(SAVE_PATH, json).map_err(|err| format!("Could not write save file: {}.", err))?;

    Ok(SAVE_PATH.to_owned())
}

pub fn load_game() -> Result<SavePayload, String> {
    let json = fs::read_to_string(SAVE_PATH)
        .map_err(|err| format!("Could not read save file: {}.", err))?;
    serde_json::from_str(&json).map_err(|err| format!("Could not parse save file: {}.", err))
}

pub fn current_save_path() -> &'static str {
    SAVE_PATH
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
            let path = Path::new(current_save_path());
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
        assert_eq!(restored.run_state.inventory.len(), run.inventory.len());
        assert_eq!(restored.run_state.equipped_item_id, run.equipped_item_id);
    }
}
