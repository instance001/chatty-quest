use std::fs;

use serde::{Deserialize, Serialize};

use crate::app_paths;
use crate::data::datapacks::{DatapackBundle, DatapackCatalog, load_datapack_bundle_by_folder};
use crate::game::{GameEvent, RunState};
use crate::runtime::SavePayload;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionSetupSnapshot {
    pub selected_datapack: String,
    pub difficulty: f32,
    pub chaos_mode: f32,
    pub fog_mode: String,
    pub dm_capsule: String,
    pub cpu_helper_model: String,
    pub gpu_narrator_model: String,
    pub narrator_backend: String,
    pub narrator_endpoint: String,
    pub byo_provider: String,
    pub byo_model: String,
    pub byo_endpoint: String,
    pub narration_preferences: crate::game::narrator::NarrationPreferences,
    pub allow_narrator_generated_additions: bool,
    pub generated_proposal_kinds: Vec<String>,
    pub play_preset: String,
}

impl Default for SessionSetupSnapshot {
    fn default() -> Self {
        Self {
            selected_datapack: String::new(),
            difficulty: 0.35,
            chaos_mode: 0.10,
            fog_mode: "Known".to_owned(),
            dm_capsule: "Hostile Meatgrinder".to_owned(),
            cpu_helper_model: "Reserved: Bookkeeper / Summary Helper".to_owned(),
            gpu_narrator_model: String::new(),
            narrator_backend: "Mock Narrator".to_owned(),
            narrator_endpoint: String::new(),
            byo_provider: "OpenAI".to_owned(),
            byo_model: "gpt-5-mini".to_owned(),
            byo_endpoint: "https://api.openai.com/v1/chat/completions".to_owned(),
            narration_preferences: crate::game::narrator::NarrationPreferences::default(),
            allow_narrator_generated_additions: false,
            generated_proposal_kinds: vec!["item".to_owned()],
            play_preset: "rules_first".to_owned(),
        }
    }
}

pub fn load_title_setup() -> Result<Option<SessionSetupSnapshot>, String> {
    load_title_setup_at(&app_paths::path("runtime/config/title_setup.json"))
}

pub fn save_title_setup(setup: &SessionSetupSnapshot) -> Result<(), String> {
    save_title_setup_at(&app_paths::path("runtime/config/title_setup.json"), setup)
}

fn load_title_setup_at(path: &std::path::Path) -> Result<Option<SessionSetupSnapshot>, String> {
    let json = match fs::read_to_string(path) {
        Ok(json) => json,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Could not read title settings: {error}")),
    };
    serde_json::from_str(&json)
        .map(Some)
        .map_err(|error| format!("Could not parse title settings: {error}"))
}

fn save_title_setup_at(path: &std::path::Path, setup: &SessionSetupSnapshot) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create title settings folder: {error}"))?;
    }
    let json = serde_json::to_string_pretty(setup)
        .map_err(|error| format!("Could not serialize title settings: {error}"))?;
    fs::write(path, json).map_err(|error| format!("Could not write title settings: {error}"))
}

#[derive(Clone, Debug)]
pub struct LoadedSessionState {
    pub save_version: u32,
    pub setup: SessionSetupSnapshot,
    pub active_tab_key: String,
    pub log_lines: Vec<String>,
    pub diagnostic_events: Vec<GameEvent>,
    pub run_state: RunState,
}

#[derive(Debug)]
pub enum RestoreBundleError {
    MissingDatapack,
    InvalidBundle(Vec<String>),
}

pub fn build_save_payload(
    save_version: u32,
    setup: &SessionSetupSnapshot,
    active_tab_key: &str,
    log_lines: &[String],
    diagnostic_events: &[GameEvent],
    run_state: &RunState,
) -> SavePayload {
    SavePayload {
        save_version,
        selected_datapack: setup.selected_datapack.clone(),
        difficulty: setup.difficulty,
        chaos_mode: setup.chaos_mode,
        fog_mode: setup.fog_mode.clone(),
        dm_capsule: setup.dm_capsule.clone(),
        cpu_helper_model: setup.cpu_helper_model.clone(),
        gpu_narrator_model: setup.gpu_narrator_model.clone(),
        narrator_backend: setup.narrator_backend.clone(),
        narrator_endpoint: setup.narrator_endpoint.clone(),
        byo_provider: setup.byo_provider.clone(),
        byo_model: setup.byo_model.clone(),
        byo_endpoint: setup.byo_endpoint.clone(),
        narration_preferences: setup.narration_preferences.clone(),
        allow_narrator_generated_additions: setup.allow_narrator_generated_additions,
        generated_proposal_kinds: setup.generated_proposal_kinds.clone(),
        play_preset: setup.play_preset.clone(),
        active_tab: active_tab_key.to_owned(),
        log_lines: log_lines.to_vec(),
        diagnostic_events: diagnostic_events.to_vec(),
        run_state: run_state.clone(),
    }
}

pub fn unpack_save_payload(payload: SavePayload) -> LoadedSessionState {
    LoadedSessionState {
        save_version: payload.save_version,
        setup: SessionSetupSnapshot {
            selected_datapack: payload.selected_datapack,
            difficulty: payload.difficulty,
            chaos_mode: payload.chaos_mode,
            fog_mode: payload.fog_mode,
            dm_capsule: payload.dm_capsule,
            cpu_helper_model: payload.cpu_helper_model,
            gpu_narrator_model: payload.gpu_narrator_model,
            narrator_backend: payload.narrator_backend,
            narrator_endpoint: payload.narrator_endpoint,
            byo_provider: payload.byo_provider,
            byo_model: payload.byo_model,
            byo_endpoint: payload.byo_endpoint,
            narration_preferences: payload.narration_preferences,
            allow_narrator_generated_additions: payload.allow_narrator_generated_additions,
            generated_proposal_kinds: payload.generated_proposal_kinds,
            play_preset: payload.play_preset,
        },
        active_tab_key: payload.active_tab,
        log_lines: payload.log_lines,
        diagnostic_events: payload.diagnostic_events,
        run_state: payload.run_state,
    }
}

pub fn restore_bundle_for_run(
    datapacks: &DatapackCatalog,
    run_state: &RunState,
) -> Result<DatapackBundle, RestoreBundleError> {
    let Some(record) = datapacks
        .valid
        .iter()
        .find(|record| record.summary.id == run_state.datapack_id)
    else {
        return Err(RestoreBundleError::MissingDatapack);
    };

    load_datapack_bundle_by_folder(&record.folder_name).map_err(RestoreBundleError::InvalidBundle)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{SessionSetupSnapshot, load_title_setup_at, save_title_setup_at};

    #[test]
    fn title_setup_roundtrip_preserves_selected_local_narrator_configuration() {
        let directory = std::env::temp_dir().join(format!(
            "chatty_quest_title_setup_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ));
        let path = directory.join("title_setup.json");
        let setup = SessionSetupSnapshot {
            selected_datapack: "station_smoke_test".to_owned(),
            chaos_mode: 1.0,
            gpu_narrator_model: "local-narrator.gguf".to_owned(),
            narrator_backend: "Local llama.cpp / GGUF".to_owned(),
            narrator_endpoint: "http://127.0.0.1:18123/v1/chat/completions".to_owned(),
            generated_proposal_kinds: vec!["item".to_owned(), "npc".to_owned()],
            ..SessionSetupSnapshot::default()
        };

        save_title_setup_at(&path, &setup).expect("title settings should save");
        assert_eq!(
            load_title_setup_at(&path).expect("title settings should load"),
            Some(setup)
        );

        std::fs::remove_dir_all(directory).expect("test title settings should clean up");
    }
}
