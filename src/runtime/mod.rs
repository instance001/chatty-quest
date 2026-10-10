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
    #[serde(default = "default_narrator_backend")]
    pub narrator_backend: String,
    #[serde(default = "default_narrator_endpoint")]
    pub narrator_endpoint: String,
    #[serde(default = "default_byo_provider")]
    pub byo_provider: String,
    #[serde(default = "default_byo_model")]
    pub byo_model: String,
    #[serde(default = "default_byo_endpoint")]
    pub byo_endpoint: String,
    #[serde(default)]
    pub narration_preferences: crate::game::narrator::NarrationPreferences,
    #[serde(default)]
    pub allow_narrator_generated_additions: bool,
    #[serde(default = "default_generated_proposal_kinds")]
    pub generated_proposal_kinds: Vec<String>,
    #[serde(default = "default_play_preset")]
    pub play_preset: String,
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
    parse_save_payload(&json)
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

fn default_play_preset() -> String {
    "rules_first".to_owned()
}

fn default_generated_proposal_kinds() -> Vec<String> {
    vec!["item".to_owned()]
}

fn default_narrator_endpoint() -> String {
    "http://127.0.0.1:8081/v1/chat/completions".to_owned()
}

fn default_narrator_backend() -> String {
    "Mock Narrator".to_owned()
}

fn default_byo_provider() -> String {
    "OpenAI".to_owned()
}

fn default_byo_model() -> String {
    "gpt-5-mini".to_owned()
}

fn default_byo_endpoint() -> String {
    "https://api.openai.com/v1/chat/completions".to_owned()
}

fn parse_save_payload(json: &str) -> Result<SavePayload, String> {
    let payload: SavePayload =
        serde_json::from_str(json).map_err(|err| format!("Could not parse save file: {}.", err))?;
    migrate_save_payload(payload)
}

fn migrate_save_payload(payload: SavePayload) -> Result<SavePayload, String> {
    match payload.save_version {
        SAVE_VERSION => Ok(payload),
        0 => Err(format!(
            "Unsupported save file version 0. Current save version is {}.",
            SAVE_VERSION
        )),
        version if version > SAVE_VERSION => Err(format!(
            "Save file version {} is newer than this app supports. Current save version is {}.",
            version, SAVE_VERSION
        )),
        version => Err(format!(
            "Unsupported save file version {}. Current save version is {}.",
            version, SAVE_VERSION
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use crate::data::datapacks::load_datapack_bundle_by_folder;
    use crate::game::generate_new_run;
    use crate::game::state::{
        AbilityGrant, AbilityGrantSource, EffectDuration, EffectTrigger, InstanceLifecycle,
        ItemOwner, ObjectiveBucket, ObjectiveState, StatusEffectInstance, StatusOwner,
        TemporaryExitInstance,
    };

    use super::{
        SavePayload, current_save_path, current_save_version, load_game, parse_save_payload,
        save_game,
    };

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
        run.max_hp = 14;
        run.active_objective.completed = true;
        run.locked_locations.remove("garage");
        run.broken_locked_locations.insert("back_garden".to_owned());
        run.barricaded_locations.insert("front_verandah".to_owned());
        run.turn_index = 42;
        run.noise_level = 2;
        run.noise_spawn_count = 1;
        run.progress_pools.insert("necromancy".to_owned(), 9);
        run.actor_progress_pools.insert(
            "save_fixture_npc".to_owned(),
            std::collections::HashMap::from([("necromancy".to_owned(), 6)]),
        );
        run.status_effects.push(StatusEffectInstance {
            instance_id: "save_fixture_entity_status".to_owned(),
            template_id: "save_fixture_entity_status".to_owned(),
            owner: StatusOwner::Entity,
            owner_id: Some("save_fixture_npc".to_owned()),
            trigger: EffectTrigger::Combat,
            duration: EffectDuration::Run,
            additive_damage: 2,
            additive_healing: 0,
            timing_cost_delta: 0,
            timing_action_kinds: Vec::new(),
            blocked_action_kinds: Vec::new(),
            required_target_tags: Vec::new(),
            required_context_tags: Vec::new(),
            lifecycle: InstanceLifecycle::Active,
        });
        run.actor_noise_targets.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "kitchen".to_owned(),
        );
        run.actor_origins.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "back_garden".to_owned(),
        );
        run.actor_searching
            .insert("noise_spawn_1_shambler_front_gate".to_owned());
        run.actor_sight_targets.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "front_verandah".to_owned(),
        );
        run.actor_sight_subjects.insert(
            "noise_spawn_1_shambler_front_gate".to_owned(),
            "player".to_owned(),
        );
        run.actor_sight_delays
            .insert("noise_spawn_1_shambler_front_gate".to_owned(), 1);
        run.ability_grants.push(AbilityGrant {
            instance_id: "save_fixture_grant".to_owned(),
            template_id: "grave_bolt".to_owned(),
            owner: ItemOwner::Player,
            remaining_uses: Some(2),
            expires_at_turn: Some(49),
            ready_at_turn: 47,
            source: AbilityGrantSource::Temporary,
        });
        run.ability_grants.push(AbilityGrant {
            instance_id: "save_fixture_reward".to_owned(),
            template_id: "bone_lance".to_owned(),
            owner: ItemOwner::Player,
            remaining_uses: None,
            expires_at_turn: None,
            ready_at_turn: 0,
            source: AbilityGrantSource::Granted,
        });
        run.additional_objectives.push(ObjectiveState {
            instance_id: "narrator_objective_1".to_owned(),
            template_id: "scratch_objective_1".to_owned(),
            lifecycle: InstanceLifecycle::Active,
            bucket: ObjectiveBucket::Active,
            id: "narrator_objective_1".to_owned(),
            name: "Reach the laundry".to_owned(),
            description: "A saved objective.".to_owned(),
            target_boss_id: None,
            required_item_id: None,
            required_location_id: Some("laundry".to_owned()),
            completed: false,
        });
        run.temporary_exits.push(TemporaryExitInstance {
            instance_id: "narrator_exit_1".to_owned(),
            template_id: "scratch_exit_1".to_owned(),
            from_location_id: "front_verandah".to_owned(),
            to_location_id: "laundry".to_owned(),
            locked: false,
            lifecycle: InstanceLifecycle::Active,
            provenance: "narrator_approved".to_owned(),
        });

        let payload = SavePayload {
            save_version: current_save_version(),
            selected_datapack: "Property Siege Classic".to_owned(),
            difficulty: 0.35,
            chaos_mode: 0.10,
            fog_mode: "Known".to_owned(),
            dm_capsule: "Grim Survival".to_owned(),
            cpu_helper_model: "cpu-test".to_owned(),
            gpu_narrator_model: "gpu-test".to_owned(),
            narrator_backend: "Mock Narrator".to_owned(),
            narrator_endpoint: "http://127.0.0.1:8081/v1/chat/completions".to_owned(),
            byo_provider: "OpenAI".to_owned(),
            byo_model: "gpt-5-mini".to_owned(),
            byo_endpoint: "https://api.openai.com/v1/chat/completions".to_owned(),
            narration_preferences: crate::game::narrator::NarrationPreferences::default(),
            allow_narrator_generated_additions: false,
            generated_proposal_kinds: vec!["item".to_owned()],
            play_preset: "rules_first".to_owned(),
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
        assert_eq!(restored.run_state.max_hp, 14);
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
        assert_eq!(restored.run_state.turn_index, 42);
        assert_eq!(restored.run_state.noise_level, 2);
        assert_eq!(restored.run_state.noise_spawn_count, 1);
        assert_eq!(
            restored.run_state.progress_pools.get("necromancy"),
            Some(&9),
            "progression must survive save/load before unlock checks run"
        );
        assert_eq!(
            restored
                .run_state
                .actor_progress_pools
                .get("save_fixture_npc")
                .and_then(|pools| pools.get("necromancy")),
            Some(&6),
            "entity-owned progression must survive save/load independently"
        );
        assert!(restored.run_state.status_effects.iter().any(|effect| {
            effect.instance_id == "save_fixture_entity_status"
                && effect.owner == StatusOwner::Entity
                && effect.owner_id.as_deref() == Some("save_fixture_npc")
        }));
        let mut legacy_value = serde_json::to_value(&payload).expect("serialize fixture save");
        legacy_value
            .get_mut("run_state")
            .and_then(serde_json::Value::as_object_mut)
            .expect("run state object")
            .remove("actor_progress_pools");
        let legacy_json = serde_json::to_string(&legacy_value).expect("serialize legacy shape");
        let migrated = parse_save_payload(&legacy_json).expect("legacy actor ledger shape loads");
        assert!(migrated.run_state.actor_progress_pools.is_empty());
        assert_eq!(
            restored
                .run_state
                .actor_noise_targets
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("kitchen")
        );
        assert_eq!(
            restored
                .run_state
                .actor_origins
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("back_garden")
        );
        assert!(
            restored
                .run_state
                .actor_searching
                .contains("noise_spawn_1_shambler_front_gate")
        );
        assert_eq!(
            restored
                .run_state
                .actor_sight_targets
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("front_verandah")
        );
        assert_eq!(
            restored
                .run_state
                .actor_sight_subjects
                .get("noise_spawn_1_shambler_front_gate")
                .map(String::as_str),
            Some("player")
        );
        assert_eq!(
            restored
                .run_state
                .actor_sight_delays
                .get("noise_spawn_1_shambler_front_gate")
                .copied(),
            Some(1)
        );
        assert_eq!(restored.run_state.inventory.len(), run.inventory.len());
        assert_eq!(restored.run_state.equipped_item_id, run.equipped_item_id);
        assert_eq!(
            restored.run_state.world_item_instances.len(),
            run.world_item_instances.len()
        );
        assert_eq!(
            restored.run_state.enemy_instances.len(),
            run.enemy_instances.len()
        );
        assert_eq!(
            restored.run_state.location_instances.len(),
            run.location_instances.len()
        );
        assert_eq!(
            restored.run_state.active_objective.instance_id,
            run.active_objective.instance_id
        );
        assert_eq!(restored.run_state.additional_objectives.len(), 1);
        assert_eq!(restored.run_state.temporary_exits.len(), 1);
        assert!(restored.run_state.ability_grants.iter().any(|grant| {
            grant.instance_id == "save_fixture_grant"
                && grant.template_id == "grave_bolt"
                && grant.owner == ItemOwner::Player
                && grant.remaining_uses == Some(2)
                && grant.expires_at_turn == Some(49)
                && grant.ready_at_turn == 47
                && grant.source == AbilityGrantSource::Temporary
        }));
        assert!(restored.run_state.ability_grants.iter().any(|grant| {
            grant.instance_id == "save_fixture_reward"
                && grant.template_id == "bone_lance"
                && grant.owner == ItemOwner::Player
                && grant.remaining_uses.is_none()
                && grant.expires_at_turn.is_none()
                && grant.source == AbilityGrantSource::Granted
        }));
    }

    #[test]
    fn save_payload_parser_rejects_unsupported_versions() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let run = generate_new_run(&bundle).state;
        let payload = SavePayload {
            save_version: current_save_version() + 1,
            selected_datapack: "Property Siege Classic".to_owned(),
            difficulty: 0.35,
            chaos_mode: 0.10,
            fog_mode: "Known".to_owned(),
            dm_capsule: "Grim Survival".to_owned(),
            cpu_helper_model: "cpu-test".to_owned(),
            gpu_narrator_model: "gpu-test".to_owned(),
            narrator_backend: "Mock Narrator".to_owned(),
            narrator_endpoint: "http://127.0.0.1:8081/v1/chat/completions".to_owned(),
            byo_provider: "OpenAI".to_owned(),
            byo_model: "gpt-5-mini".to_owned(),
            byo_endpoint: "https://api.openai.com/v1/chat/completions".to_owned(),
            narration_preferences: crate::game::narrator::NarrationPreferences::default(),
            allow_narrator_generated_additions: false,
            generated_proposal_kinds: vec!["item".to_owned()],
            play_preset: "rules_first".to_owned(),
            active_tab: "Game".to_owned(),
            log_lines: vec!["System: test save".to_owned()],
            diagnostic_events: Vec::new(),
            run_state: run,
        };

        let json = serde_json::to_string(&payload).expect("expected test payload to serialize");
        let error = parse_save_payload(&json).expect_err("expected future save version to fail");

        assert!(error.contains("newer than this app supports"));
        assert!(error.contains("Current save version is 1"));
    }

    #[test]
    fn save_payload_parser_accepts_missing_version_as_v1() {
        let bundle = load_datapack_bundle_by_folder("property_siege_classic")
            .expect("expected property_siege_classic bundle to load");
        let run = generate_new_run(&bundle).state;
        let payload = SavePayload {
            save_version: current_save_version(),
            selected_datapack: "Property Siege Classic".to_owned(),
            difficulty: 0.35,
            chaos_mode: 0.10,
            fog_mode: "Known".to_owned(),
            dm_capsule: "Grim Survival".to_owned(),
            cpu_helper_model: "cpu-test".to_owned(),
            gpu_narrator_model: "gpu-test".to_owned(),
            narrator_backend: "Mock Narrator".to_owned(),
            narrator_endpoint: "http://127.0.0.1:8081/v1/chat/completions".to_owned(),
            byo_provider: "OpenAI".to_owned(),
            byo_model: "gpt-5-mini".to_owned(),
            byo_endpoint: "https://api.openai.com/v1/chat/completions".to_owned(),
            narration_preferences: crate::game::narrator::NarrationPreferences::default(),
            allow_narrator_generated_additions: false,
            generated_proposal_kinds: vec!["item".to_owned()],
            play_preset: "rules_first".to_owned(),
            active_tab: "Game".to_owned(),
            log_lines: vec!["System: test save".to_owned()],
            diagnostic_events: Vec::new(),
            run_state: run,
        };
        let mut value =
            serde_json::to_value(&payload).expect("expected test payload to become JSON");
        value
            .as_object_mut()
            .expect("expected save payload object")
            .remove("save_version");
        let json = serde_json::to_string(&value).expect("expected test payload to serialize");

        let restored =
            parse_save_payload(&json).expect("expected missing version to default to v1");

        assert_eq!(restored.save_version, current_save_version());
    }
}
