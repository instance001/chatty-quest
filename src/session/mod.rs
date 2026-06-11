use crate::data::datapacks::{DatapackBundle, DatapackCatalog, load_datapack_bundle_by_folder};
use crate::game::{GameEvent, RunState};
use crate::runtime::SavePayload;

#[derive(Clone, Debug)]
pub struct SessionSetupSnapshot {
    pub selected_datapack: String,
    pub difficulty: f32,
    pub chaos_mode: f32,
    pub fog_mode: String,
    pub dm_capsule: String,
    pub cpu_helper_model: String,
    pub gpu_narrator_model: String,
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
