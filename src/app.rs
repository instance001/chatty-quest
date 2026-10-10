use std::fs::File;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::app_paths;
use crate::collection::{
    CollectionItem, PlayerTemplateAdmission, PlayerTemplateKind, PlayerTemplateMechanics,
    PlayerTemplateMediaReferences, collection_item_id, load_player_templates_for_datapack,
    normalize_tags, save_player_template,
};
use crate::data::datapacks::{
    DatapackBundle, DatapackCatalog, DatapackRecord, InvalidDatapack, datapack_schema_version,
    discover_datapacks, load_datapack_bundle_by_folder,
};
use crate::diagnostics::build_diagnostic_report;
use crate::game::capabilities::{CapabilityDependentField, EngineCapability};
use crate::game::narrator::{
    CloudNarratorTransport, LiveNarratorAdapter, LlamaCppNarratorTransport,
    LocalHttpNarratorTransport, MockNarrator, NarrationPreferences, NarrationRoute, Narrator,
    UnavailableNarrator, ValidatedAbilityGrantProposal, ValidatedBossProposal,
    ValidatedEnemyProposal, ValidatedItemProposal, ValidatedNpcProposal,
    ValidatedObjectiveProposal, ValidatedStatusProposal, ValidatedTemporaryExitProposal,
    narration_hook_for_action, narration_route,
};
use crate::game::state::{AbilityGrantSource, ItemOwner, TemporaryExitInstance};
use crate::game::{
    ActorDisposition, ActorState, EffectDuration, EffectTrigger, EncounterBucket,
    EncounterInstance, GameAction, GameEvent, GeneratedRun, InstanceLifecycle, InventoryEntry,
    ItemBucket, ItemOrigin, NpcInstance, ObjectiveBucket, ObjectiveState, PlayContract, RunState,
    StatusEffectInstance, StatusOwner, action_traces, apply_action, generate_new_run,
};
use crate::media::build_media_panel_state_with_focus;
use crate::runtime::{current_save_path, current_save_version, load_game, save_game};
use crate::session::{
    RestoreBundleError, SessionSetupSnapshot, build_save_payload, load_title_setup,
    restore_bundle_for_run, save_title_setup, unpack_save_payload,
};
use crate::ui::{
    AssetViewerRequest, LibraryUiState, SelectedDatapackPreview, SetupScreenAction,
    SetupScreenModel, SplashScreenModel, show_asset_viewer, show_character_tab,
    show_diagnostics_tab, show_game_action_bar, show_game_tab, show_inventory_tab,
    show_library_tab, show_setup_screen, show_splash_screen,
};

const APP_DISPLAY_NAME: &str = "Chatty Quest";
const ENGINE_DISPLAY_NAME: &str = "RD Engine";
const FMI_DISPLAY_NAME: &str = "Fractal Media Infrastructure";
const SPLASH_PRIMARY_IMAGE_PATH: &str = "assets/ui/branding/chatty-quest-splash-screen.png";
const SPLASH_ENGINE_IMAGE_PATH: &str = "assets/ui/branding/RD-Engine-logo.png";
const SPLASH_FMI_IMAGE_PATH: &str = "assets/ui/branding/fmi-splash-wordmark.png";
const SETUP_HEADER_IMAGE_PATH: &str = "assets/ui/branding/chatty-quest-think-it-play-it.png";
const SPLASH_PRIMARY_DURATION: Duration = Duration::from_millis(2600);
const SPLASH_ENGINE_DURATION: Duration = Duration::from_millis(1400);
const SPLASH_FMI_DURATION: Duration = Duration::from_millis(1400);
const LOCAL_LLAMA_PORT_BASE: u16 = 18080;

fn discover_local_gguf_models() -> Vec<String> {
    let mut models = std::fs::read_dir(app_paths::path("models/local"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("gguf")))
            .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    models.sort();
    models
}

fn local_llama_port(model: &str) -> u16 {
    let offset = model
        .bytes()
        .fold(0_u16, |sum, byte| sum.wrapping_add(byte as u16))
        % 1000;
    LOCAL_LLAMA_PORT_BASE + offset
}

fn local_llama_endpoint(model: &str) -> String {
    format!(
        "http://127.0.0.1:{}/v1/chat/completions",
        local_llama_port(model)
    )
}

fn start_selected_local_model(model: &str) -> Result<String, String> {
    let model_path = app_paths::path("models/local").join(model);
    if !model_path.is_file() {
        return Err(format!(
            "Selected GGUF model '{}' is not present in models/local",
            model
        ));
    }
    let endpoint = local_llama_endpoint(model);
    if LlamaCppNarratorTransport::is_ready(&endpoint, Duration::from_millis(250)) {
        return Ok(endpoint);
    }
    // A previous app launch may still be loading this model.  Never start a competing host on
    // the same model-specific port; wait for the existing process to become genuinely ready.
    if LlamaCppNarratorTransport::is_available(&endpoint, Duration::from_millis(250)) {
        for _ in 0..600 {
            if LlamaCppNarratorTransport::is_ready(&endpoint, Duration::from_millis(250)) {
                return Ok(endpoint);
            }
            thread::sleep(Duration::from_millis(100));
        }
        return Err(format!(
            "Local model '{}' accepted connections but did not become ready within 60 seconds. See runtime/logs for llama.cpp details.",
            model
        ));
    }

    let server = app_paths::path("runtime/bin/windows/llama-server.exe");
    if !server.is_file() {
        return Err(format!(
            "Missing project-local llama.cpp runtime: {}",
            server.display()
        ));
    }
    let port = local_llama_port(model).to_string();
    let logs = app_paths::path("runtime/logs");
    std::fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
    let slug = model
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    let stdout = File::create(logs.join(format!("llama-{slug}.stdout.log")))
        .map_err(|error| error.to_string())?;
    let stderr = File::create(logs.join(format!("llama-{slug}.stderr.log")))
        .map_err(|error| error.to_string())?;
    let model_path_string = model_path.to_string_lossy().into_owned();
    let mut child = Command::new(&server)
        .current_dir(server.parent().unwrap_or_else(|| std::path::Path::new(".")))
        .args([
            "-m",
            model_path_string.as_str(),
            "--host",
            "127.0.0.1",
            "--port",
            port.as_str(),
            "--ctx-size",
            "4096",
            "--parallel",
            "1",
            "--gpu-layers",
            "99",
            "--reasoning",
            "off",
        ])
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|error| format!("Could not start local llama.cpp host: {error}"))?;

    for _ in 0..600 {
        if LlamaCppNarratorTransport::is_ready(&endpoint, Duration::from_millis(250)) {
            return Ok(endpoint);
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("Could not check local llama.cpp host: {error}"))?
        {
            return Err(format!(
                "Local model '{}' stopped during startup ({status}). See {}.",
                model,
                logs.join(format!("llama-{slug}.stderr.log")).display()
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "Local model '{}' did not become ready. See {}.",
        model,
        logs.join(format!("llama-{slug}.stderr.log")).display()
    ))
}

fn build_narrator(bundle: &DatapackBundle, setup: &SetupConfig) -> Box<dyn Narrator> {
    let fallback = MockNarrator::new(bundle, setup.chaos_mode);
    match setup.narrator_backend.as_str() {
        "Mock Narrator" => Box::new(fallback),
        "Local OpenAI-compatible" if !setup.narrator_endpoint.trim().is_empty() => Box::new(
            LiveNarratorAdapter::with_play_contract_chaos_and_allowed_kinds(
                LocalHttpNarratorTransport::new(&setup.narrator_endpoint, Duration::from_secs(8)),
                fallback,
                setup.play_contract(),
                setup.chaos_mode,
                setup.authorized_proposal_kinds(),
            ),
        ),
        "Local llama.cpp / GGUF" => match start_selected_local_model(&setup.gpu_narrator_model) {
            Ok(endpoint) => Box::new(
                LiveNarratorAdapter::with_play_contract_chaos_and_allowed_kinds(
                    LlamaCppNarratorTransport::new(endpoint, Duration::from_secs(90)),
                    fallback,
                    setup.play_contract(),
                    setup.chaos_mode,
                    setup.authorized_proposal_kinds(),
                ),
            ),
            Err(error) => Box::new(UnavailableNarrator::new(error)),
        },
        "BYO API Key"
            if !setup.byo_provider.trim().is_empty()
                && !setup.byo_model.trim().is_empty()
                && !setup.byo_endpoint.trim().is_empty()
                && !setup.byo_api_key.trim().is_empty() =>
        {
            Box::new(
                LiveNarratorAdapter::with_play_contract_chaos_and_allowed_kinds(
                    CloudNarratorTransport::new(
                        &setup.byo_provider,
                        &setup.byo_model,
                        &setup.byo_endpoint,
                        &setup.byo_api_key,
                        Duration::from_secs(90),
                    ),
                    fallback,
                    setup.play_contract(),
                    setup.chaos_mode,
                    setup.authorized_proposal_kinds(),
                ),
            )
        }
        "BYO API Key" => Box::new(UnavailableNarrator::new(
            "Choose a provider and model, then paste a BYO API key.",
        )),
        "None" => Box::new(UnavailableNarrator::new("Narration is off")),
        _ => Box::new(UnavailableNarrator::new(format!(
            "Unknown narrator backend '{}'",
            setup.narrator_backend
        ))),
    }
}

/// The library preserves disabled templates for later reuse, but materialization must receive an
/// already-authorized set so no template family can accidentally bypass the enable gate.
fn enabled_player_templates(templates: Vec<CollectionItem>) -> Vec<CollectionItem> {
    templates
        .into_iter()
        .filter(|template| template.enabled)
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppScreen {
    Splash,
    Setup,
    ActiveRun,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GameTab {
    Game,
    Inventory,
    Character,
    Diagnostics,
    Library,
}

struct SetupConfig {
    selected_datapack: String,
    difficulty: f32,
    chaos_mode: f32,
    fog_mode: String,
    dm_capsule: String,
    cpu_helper_model: String,
    gpu_narrator_model: String,
    narrator_backend: String,
    narrator_endpoint: String,
    byo_provider: String,
    byo_model: String,
    byo_endpoint: String,
    byo_api_key: String,
    narration_preferences: NarrationPreferences,
    allow_narrator_generated_additions: bool,
    generated_proposal_kinds: Vec<String>,
    play_preset: String,
}

impl SetupConfig {
    fn play_contract(&self) -> PlayContract {
        PlayContract::from_preset_name(&self.play_preset)
    }

    fn permits_generated_persistent_content(&self) -> bool {
        self.play_contract().allows_persistent_content() && self.allow_narrator_generated_additions
    }

    fn authorized_proposal_kinds(&self) -> Vec<String> {
        crate::game::narrator::allowed_proposal_kinds(&self.play_contract(), self.chaos_mode)
            .into_iter()
            .filter(|kind| {
                self.generated_proposal_kinds
                    .iter()
                    .any(|allowed| allowed == kind)
            })
            .collect()
    }
    fn to_snapshot(&self) -> SessionSetupSnapshot {
        SessionSetupSnapshot {
            selected_datapack: self.selected_datapack.clone(),
            difficulty: self.difficulty,
            chaos_mode: self.chaos_mode,
            fog_mode: self.fog_mode.clone(),
            dm_capsule: self.dm_capsule.clone(),
            cpu_helper_model: self.cpu_helper_model.clone(),
            gpu_narrator_model: self.gpu_narrator_model.clone(),
            narrator_backend: self.narrator_backend.clone(),
            narrator_endpoint: self.narrator_endpoint.clone(),
            byo_provider: self.byo_provider.clone(),
            byo_model: self.byo_model.clone(),
            byo_endpoint: self.byo_endpoint.clone(),
            narration_preferences: self.narration_preferences.clone(),
            allow_narrator_generated_additions: self.allow_narrator_generated_additions,
            generated_proposal_kinds: self.generated_proposal_kinds.clone(),
            play_preset: self.play_preset.clone(),
        }
    }

    fn apply_snapshot(&mut self, snapshot: SessionSetupSnapshot) {
        self.selected_datapack = snapshot.selected_datapack;
        self.difficulty = snapshot.difficulty;
        self.chaos_mode = snapshot.chaos_mode;
        self.fog_mode = snapshot.fog_mode;
        self.dm_capsule = snapshot.dm_capsule;
        self.cpu_helper_model = snapshot.cpu_helper_model;
        self.gpu_narrator_model = snapshot.gpu_narrator_model;
        self.narrator_backend = snapshot.narrator_backend;
        self.narrator_endpoint = snapshot.narrator_endpoint;
        self.byo_provider = snapshot.byo_provider;
        self.byo_model = snapshot.byo_model;
        self.byo_endpoint = snapshot.byo_endpoint;
        self.narration_preferences = snapshot.narration_preferences;
        self.allow_narrator_generated_additions = snapshot.allow_narrator_generated_additions;
        self.generated_proposal_kinds = snapshot.generated_proposal_kinds;
        self.play_preset = snapshot.play_preset;
    }
}

struct UiSessionState {
    active_tab: GameTab,
    current_input: String,
    asset_viewer_request: Option<AssetViewerRequest>,
    media_focus_entity_id: Option<String>,
    library: LibraryUiState,
}

struct SplashState {
    started_at: Instant,
}

#[derive(Clone)]
struct NarratorItemProposal {
    name: String,
    description: String,
    damage: i32,
    condition: Option<String>,
    tags: Vec<String>,
    actor: ProposalActorConfiguration,
}

#[derive(Clone)]
struct NarratorStatusProposal {
    name: String,
    trigger: String,
    duration: String,
    additive_damage: i32,
    additive_healing: i32,
    required_target_tags: Vec<String>,
    required_context_tags: Vec<String>,
}

#[derive(Clone)]
struct NarratorAbilityGrantProposal {
    template_id: String,
    remaining_uses: Option<u32>,
    duration_turns: Option<u64>,
    recipient_id: String,
}

#[derive(Clone)]
struct NarratorNpcProposal {
    name: String,
    description: String,
    faction: String,
    tags: Vec<String>,
    actor: ProposalActorConfiguration,
}

/// The editable, host-owned actor envelope for runtime proposals. The narrator never supplies
/// these fields: it fills identity/form values, the host defaults this envelope, and the player
/// sees and may adjust the mechanically supported composition before reducer admission.
#[derive(Clone)]
struct ProposalActorConfiguration {
    has_health: bool,
    max_hp: i32,
    can_engage: bool,
    actor_damage: i32,
    can_hear: bool,
    can_see: bool,
    can_move: bool,
    can_carry_inventory: bool,
    can_spawn_from_noise: bool,
    can_track_noise: bool,
    can_track_sight: bool,
    disposition: ActorDisposition,
}

impl Default for ProposalActorConfiguration {
    fn default() -> Self {
        Self {
            has_health: false,
            max_hp: 10,
            can_engage: false,
            actor_damage: 0,
            can_hear: false,
            can_see: false,
            can_move: false,
            can_carry_inventory: false,
            can_spawn_from_noise: false,
            can_track_noise: false,
            can_track_sight: false,
            disposition: ActorDisposition::Neutral,
        }
    }
}

impl ProposalActorConfiguration {
    fn from_narrator(actor: crate::game::narrator::NarratorActorConfiguration) -> Self {
        let mut configuration = Self {
            has_health: actor.has_health,
            max_hp: actor.max_hp,
            can_engage: actor.can_engage,
            actor_damage: actor.actor_damage,
            can_hear: actor.can_hear,
            can_see: actor.can_see,
            can_move: actor.can_move,
            can_carry_inventory: actor.can_carry_inventory,
            can_spawn_from_noise: actor.can_spawn_from_noise,
            can_track_noise: actor.can_track_noise,
            can_track_sight: actor.can_track_sight,
            disposition: match actor.disposition.as_str() {
                "friendly" => ActorDisposition::Friendly,
                "hostile" => ActorDisposition::Hostile,
                _ => ActorDisposition::Neutral,
            },
        };
        configuration.normalize_dependencies();
        configuration
    }

    fn capability_enabled(&self, capability: EngineCapability) -> bool {
        match capability {
            EngineCapability::Health => self.has_health,
            EngineCapability::Combat => self.can_engage,
            EngineCapability::Hearing => self.can_hear,
            EngineCapability::Sight => self.can_see,
            EngineCapability::Movement => self.can_move,
            EngineCapability::Inventory => self.can_carry_inventory,
            EngineCapability::NoiseSpawn => self.can_spawn_from_noise,
            EngineCapability::NoiseTrack => self.can_track_noise,
            EngineCapability::SightTrack => self.can_track_sight,
        }
    }

    fn set_capability_enabled(&mut self, capability: EngineCapability, enabled: bool) {
        match capability {
            EngineCapability::Health => self.has_health = enabled,
            EngineCapability::Combat => self.can_engage = enabled,
            EngineCapability::Hearing => self.can_hear = enabled,
            EngineCapability::Sight => self.can_see = enabled,
            EngineCapability::Movement => self.can_move = enabled,
            EngineCapability::Inventory => self.can_carry_inventory = enabled,
            EngineCapability::NoiseSpawn => self.can_spawn_from_noise = enabled,
            EngineCapability::NoiseTrack => self.can_track_noise = enabled,
            EngineCapability::SightTrack => self.can_track_sight = enabled,
        }
    }

    fn prerequisites_met(&self, capability: EngineCapability) -> bool {
        capability
            .prerequisites()
            .iter()
            .all(|prerequisite| self.capability_enabled(*prerequisite))
    }

    /// Disabling a root retracts every dependent capability before the reducer sees the
    /// proposal. Values may remain as harmless draft defaults, but no disabled branch becomes
    /// authoritative actor state.
    fn normalize_dependencies(&mut self) {
        for capability in EngineCapability::ALL {
            if !self.prerequisites_met(capability) {
                self.set_capability_enabled(capability, false);
            }
        }
    }

    fn effective_actor(
        &self,
        label: &str,
    ) -> Result<(Option<ActorState>, ActorDisposition), String> {
        let capabilities = EngineCapability::ALL
            .into_iter()
            .filter(|capability| self.capability_enabled(*capability))
            .map(|capability| capability.id().to_owned())
            .collect::<Vec<_>>();
        if capabilities.is_empty() {
            return Ok((None, self.disposition.clone()));
        }
        let hp = self.has_health.then_some(self.max_hp);
        let damage = self.can_engage.then_some(self.actor_damage);
        let mut errors = Vec::new();
        crate::game::capabilities::validate_selection(
            "Runtime proposal",
            label,
            &capabilities,
            true,
            hp.is_some_and(|value| value > 0),
            damage.is_some_and(|value| value >= 0),
            self.can_hear,
            self.can_see,
            self.can_move,
            &mut errors,
        );
        if !errors.is_empty() {
            return Err(errors.join(" "));
        }
        let mut actor = ActorState::configured(
            hp,
            damage,
            self.can_hear,
            self.can_see,
            self.can_move,
            self.can_engage,
        );
        actor.apply_declared_capabilities(&capabilities);
        Ok((Some(actor), self.disposition.clone()))
    }
}

fn show_actor_capability_contract(ui: &mut egui::Ui, contract: &mut ProposalActorConfiguration) {
    contract.normalize_dependencies();
    ui.separator();
    ui.strong("Effective host capability contract");
    ui.small(
        EngineCapability::ALL
            .into_iter()
            .map(|capability| {
                format!(
                    "{} {}",
                    capability.ui_label(),
                    yes_no(contract.capability_enabled(capability))
                )
            })
            .collect::<Vec<_>>()
            .join(" · "),
    );
    ui.small("Each checkbox is a host-owned schema root. Enabling it reveals its applicable dependent contract; disabling it retracts dependent mechanics before admission.");
    for capability in EngineCapability::ALL {
        if !contract.prerequisites_met(capability) {
            continue;
        }
        let mut enabled = contract.capability_enabled(capability);
        ui.checkbox(&mut enabled, capability.ui_label());
        contract.set_capability_enabled(capability, enabled);
        if !enabled {
            continue;
        }
        for field in capability.dependent_fields() {
            ui.horizontal(|ui| match field {
                CapabilityDependentField::MaxHp => {
                    ui.label(field.ui_label());
                    ui.add(egui::DragValue::new(&mut contract.max_hp).range(1..=999));
                }
                CapabilityDependentField::ActorDamage => {
                    ui.label(field.ui_label());
                    ui.add(egui::DragValue::new(&mut contract.actor_damage).range(0..=999));
                }
            });
        }
    }
    contract.normalize_dependencies();
    ui.horizontal(|ui| {
        ui.label("Disposition");
        egui::ComboBox::from_id_salt("proposal_actor_disposition")
            .selected_text(match contract.disposition {
                ActorDisposition::Friendly => "Friendly",
                ActorDisposition::Neutral => "Neutral",
                ActorDisposition::Hostile => "Hostile",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut contract.disposition,
                    ActorDisposition::Friendly,
                    "Friendly",
                );
                ui.selectable_value(
                    &mut contract.disposition,
                    ActorDisposition::Neutral,
                    "Neutral",
                );
                ui.selectable_value(
                    &mut contract.disposition,
                    ActorDisposition::Hostile,
                    "Hostile",
                );
            });
    });
    ui.small("Capability prerequisites are checked by the host on admission (combat needs health; tracking needs the relevant sense and movement).");
}

const fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

fn show_locked_encounter_contract(ui: &mut egui::Ui, max_hp: i32, damage: i32) {
    ui.separator();
    ui.strong("Effective host capability contract");
    ui.label(format!("Has health: Yes (max HP: {})", max_hp));
    ui.label(format!("Can engage enemies: Yes (damage: {})", damage));
    ui.label("Can be picked up: No · Can be equipped: No");
    ui.label("Can move: No · Can hear: No · Can see: No");
    ui.small("This proposal family currently materializes as a placed combat encounter. Its actor/perception envelope is locked to the host encounter contract.");
}

/// Proposal tags are descriptive host metadata, not hidden mechanics. Keeping them editable on
/// the approval card makes the runtime inspection surface and the saved template agree.
fn show_editable_proposal_tags(ui: &mut egui::Ui, tags: &mut Vec<String>) {
    let mut text = tags.join(", ");
    ui.horizontal(|ui| {
        ui.label("Tags");
        if ui.text_edit_singleline(&mut text).changed() {
            *tags = normalize_tags(text.split(',').map(str::to_owned).collect());
        }
    });
    ui.small("Comma-separated descriptive tags. They are visible in inspection and media pools.");
}

#[derive(Clone)]
struct NarratorEnemyProposal {
    name: String,
    description: String,
    max_hp: i32,
    damage: i32,
    tags: Vec<String>,
}
#[derive(Clone)]
struct NarratorBossProposal {
    name: String,
    description: String,
    max_hp: i32,
    damage: i32,
    tags: Vec<String>,
}
#[derive(Clone)]
struct NarratorObjectiveProposal {
    name: String,
    description: String,
    completion_key: String,
}
#[derive(Clone)]
struct NarratorTemporaryExitProposal {
    name: String,
    description: String,
    from_location_id: String,
    to_location_id: String,
    locked: bool,
}

/// A reducer action is committed before this exists.  The worker carries presentation output and
/// the narrator object back to the UI thread so slow local/hosted completion can never make the
/// desktop window stop responding.
struct NarrationTaskResult {
    narrator: Box<dyn Narrator>,
    lines: Vec<String>,
    item_proposals: Vec<ValidatedItemProposal>,
    status_proposals: Vec<ValidatedStatusProposal>,
    npc_proposals: Vec<ValidatedNpcProposal>,
    enemy_proposals: Vec<ValidatedEnemyProposal>,
    boss_proposals: Vec<ValidatedBossProposal>,
    objective_proposals: Vec<ValidatedObjectiveProposal>,
    temporary_exit_proposals: Vec<ValidatedTemporaryExitProposal>,
    ability_grant_proposals: Vec<ValidatedAbilityGrantProposal>,
    diagnostics: Vec<String>,
    action_traces: Vec<String>,
}

pub struct ChattyQuestApp {
    screen: AppScreen,
    setup: SetupConfig,
    ui_session: UiSessionState,
    splash: SplashState,
    datapacks: DatapackCatalog,
    current_bundle: Option<DatapackBundle>,
    current_run: Option<RunState>,
    narrator: Option<Box<dyn Narrator>>,
    narration_task: Option<Receiver<NarrationTaskResult>>,
    mock_narrator: Option<MockNarrator>,
    log_lines: Vec<String>,
    diagnostic_events: Vec<GameEvent>,
    loaded_save_version: Option<u32>,
    pending_narrator_item: Option<NarratorItemProposal>,
    queued_narrator_items: Vec<NarratorItemProposal>,
    pending_narrator_status: Option<NarratorStatusProposal>,
    queued_narrator_statuses: Vec<NarratorStatusProposal>,
    pending_narrator_npc: Option<NarratorNpcProposal>,
    queued_narrator_npcs: Vec<NarratorNpcProposal>,
    pending_narrator_enemy: Option<NarratorEnemyProposal>,
    queued_narrator_enemies: Vec<NarratorEnemyProposal>,
    pending_narrator_boss: Option<NarratorBossProposal>,
    queued_narrator_bosses: Vec<NarratorBossProposal>,
    pending_narrator_objective: Option<NarratorObjectiveProposal>,
    queued_narrator_objectives: Vec<NarratorObjectiveProposal>,
    pending_narrator_temporary_exit: Option<NarratorTemporaryExitProposal>,
    queued_narrator_temporary_exits: Vec<NarratorTemporaryExitProposal>,
    pending_narrator_ability_grant: Option<NarratorAbilityGrantProposal>,
    queued_narrator_ability_grants: Vec<NarratorAbilityGrantProposal>,
}

impl ChattyQuestApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let datapacks = discover_datapacks();
        let selected_datapack = datapacks
            .valid
            .first()
            .map(|record| record.folder_name.clone())
            .or_else(|| {
                datapacks
                    .invalid
                    .first()
                    .map(|record| record.folder_name.clone())
            })
            .unwrap_or_else(|| "No cartridges found".to_owned());
        let fallback_datapack = selected_datapack.clone();
        let mut setup = SetupConfig {
            selected_datapack,
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
            byo_api_key: String::new(),
            narration_preferences: NarrationPreferences::default(),
            allow_narrator_generated_additions: false,
            generated_proposal_kinds: vec!["item".to_owned()],
            play_preset: "rules_first".to_owned(),
        };
        if let Ok(Some(saved_setup)) = load_title_setup() {
            setup.apply_snapshot(saved_setup);
        }
        let saved_datapack_exists = datapacks.valid.iter().any(|record| {
            record.folder_name == setup.selected_datapack
                || record.summary.display_name == setup.selected_datapack
        }) || datapacks
            .invalid
            .iter()
            .any(|record| record.folder_name == setup.selected_datapack);
        if !saved_datapack_exists {
            setup.selected_datapack = fallback_datapack;
        }

        Self {
            screen: AppScreen::Splash,
            setup,
            ui_session: UiSessionState {
                active_tab: GameTab::Game,
                current_input: String::new(),
                asset_viewer_request: None,
                media_focus_entity_id: None,
                library: LibraryUiState::default(),
            },
            splash: SplashState {
                started_at: Instant::now(),
            },
            datapacks,
            current_bundle: None,
            current_run: None,
            narrator: None,
            narration_task: None,
            mock_narrator: None,
            log_lines: vec![
                format!(
                    "{} is ready for a local run on the {}.",
                    APP_DISPLAY_NAME, ENGINE_DISPLAY_NAME
                ),
                "Datapack discovery reads the active app root's assets/datapacks folder."
                    .to_owned(),
                "Generate a scenario or load the current save to enter the tabbed game shell."
                    .to_owned(),
            ],
            diagnostic_events: Vec::new(),
            loaded_save_version: None,
            pending_narrator_item: None,
            queued_narrator_items: Vec::new(),
            pending_narrator_status: None,
            queued_narrator_statuses: Vec::new(),
            pending_narrator_npc: None,
            queued_narrator_npcs: Vec::new(),
            pending_narrator_enemy: None,
            queued_narrator_enemies: Vec::new(),
            pending_narrator_boss: None,
            queued_narrator_bosses: Vec::new(),
            pending_narrator_objective: None,
            queued_narrator_objectives: Vec::new(),
            pending_narrator_temporary_exit: None,
            queued_narrator_temporary_exits: Vec::new(),
            pending_narrator_ability_grant: None,
            queued_narrator_ability_grants: Vec::new(),
        }
    }

    fn show_splash_screen(&mut self, ctx: &egui::Context) {
        let elapsed = self.splash.started_at.elapsed();
        let total_duration = SPLASH_PRIMARY_DURATION + SPLASH_ENGINE_DURATION + SPLASH_FMI_DURATION;

        if elapsed >= total_duration
            || ctx.input(|input| {
                input.pointer.any_click()
                    || input.key_pressed(egui::Key::Escape)
                    || input.key_pressed(egui::Key::Enter)
                    || input.key_pressed(egui::Key::Space)
            })
        {
            self.screen = AppScreen::Setup;
            return;
        }

        ctx.request_repaint_after(Duration::from_millis(16));

        let (title, subtitle, supporting_line, footer, image_path) =
            if elapsed < SPLASH_PRIMARY_DURATION {
                (
                    APP_DISPLAY_NAME,
                    "Think it. Play it.",
                    Some("A deterministic chat-forward adventure engine."),
                    "Press Space, Enter, Esc, or click to skip",
                    asset_if_present(SPLASH_PRIMARY_IMAGE_PATH),
                )
            } else if elapsed < SPLASH_PRIMARY_DURATION + SPLASH_ENGINE_DURATION {
                (
                    ENGINE_DISPLAY_NAME,
                    "Radiant Determinism Engine",
                    Some("Deterministic truth under the hood."),
                    "Press Space, Enter, Esc, or click to skip",
                    asset_if_present(SPLASH_ENGINE_IMAGE_PATH),
                )
            } else {
                (
                    FMI_DISPLAY_NAME,
                    "Publisher / steward",
                    None,
                    "Press Space, Enter, Esc, or click to continue",
                    asset_if_present(SPLASH_FMI_IMAGE_PATH),
                )
            };

        show_splash_screen(
            ctx,
            SplashScreenModel {
                title,
                subtitle,
                supporting_line,
                footer,
                image_path,
                progress: (elapsed.as_secs_f32() / total_duration.as_secs_f32()).clamp(0.0, 1.0),
            },
        );
    }

    fn show_setup_screen(&mut self, ctx: &egui::Context) {
        let setup_before = self.setup.to_snapshot();
        let selected_record =
            self.selected_datapack_record()
                .map(|record| SelectedDatapackPreview {
                    id: record.summary.id.clone(),
                    folder_name: record.folder_name.clone(),
                    version: record.summary.version.clone(),
                    author: record.summary.author.clone(),
                    description: record.summary.description.clone(),
                    primary_scenario: record.summary.primary_scenario.clone(),
                    location_count: record.summary.location_count,
                    item_count: record.summary.item_count,
                    enemy_count: record.summary.enemy_count,
                    boss_count: record.summary.boss_count,
                    objective_count: record.summary.objective_count,
                    narrator_brief_count: record.summary.narrator_brief_count,
                    media_reference_count: record.summary.media_reference_count,
                    sensory_template_count: record.summary.sensory_template_count,
                    boundary_response: record.summary.boundary_response.clone(),
                    dm_style_preview: record.summary.dm_style_preview.clone(),
                    world_tone_preview: record.summary.world_tone_preview.clone(),
                    required_engine_version: record.summary.required_engine_version.clone(),
                    license: record.summary.license.clone(),
                    attribution: record.summary.attribution.clone(),
                    capabilities: record.summary.capabilities.clone(),
                });
        let selected_invalid = self.selected_invalid_datapack().map(|record| {
            crate::ui::SelectedInvalidDatapackPreview {
                folder_name: record.folder_name.clone(),
                display_name: record.display_name.clone(),
                version: record.version.clone(),
                errors: record.errors.clone(),
            }
        });
        let selected_label = self.selected_datapack_label();
        let play_contract_summary = self.setup.play_contract().control_summary();
        let local_narrator_models = discover_local_gguf_models();
        if self.setup.gpu_narrator_model.is_empty()
            && let Some(model) = local_narrator_models.first()
        {
            self.setup.gpu_narrator_model = model.clone();
        }
        let action = show_setup_screen(
            ctx,
            SetupScreenModel {
                selected_datapack: &mut self.setup.selected_datapack,
                selected_datapack_label: selected_label,
                difficulty: &mut self.setup.difficulty,
                chaos_mode: &mut self.setup.chaos_mode,
                fog_mode: &mut self.setup.fog_mode,
                dm_capsule: &mut self.setup.dm_capsule,
                cpu_helper_model: &mut self.setup.cpu_helper_model,
                gpu_narrator_model: &mut self.setup.gpu_narrator_model,
                narrator_backend: &mut self.setup.narrator_backend,
                narrator_endpoint: &mut self.setup.narrator_endpoint,
                byo_provider: &mut self.setup.byo_provider,
                byo_model: &mut self.setup.byo_model,
                byo_endpoint: &mut self.setup.byo_endpoint,
                byo_api_key: &mut self.setup.byo_api_key,
                narration_preferences: &mut self.setup.narration_preferences,
                local_narrator_models,
                allow_narrator_generated_additions: &mut self
                    .setup
                    .allow_narrator_generated_additions,
                generated_proposal_kinds: &mut self.setup.generated_proposal_kinds,
                play_preset: &mut self.setup.play_preset,
                play_contract_summary,
                datapacks: &self.datapacks,
                selected_record,
                selected_invalid,
                header_image_path: asset_if_present(SETUP_HEADER_IMAGE_PATH),
            },
        );

        match action {
            SetupScreenAction::None => {}
            SetupScreenAction::GenerateGame => self.start_new_run(),
            SetupScreenAction::LoadGame => self.load_saved_run(),
            SetupScreenAction::RefreshCartridges => self.refresh_datapacks(),
        }
        if self.setup.to_snapshot() != setup_before {
            self.persist_title_setup();
        }
    }

    fn persist_title_setup(&self) {
        if let Err(error) = save_title_setup(&self.setup.to_snapshot()) {
            eprintln!("{error}");
        }
    }

    fn show_game_shell(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Setup").clicked() {
                    self.screen = AppScreen::Setup;
                }

                if ui.button("Save").clicked() {
                    self.save_current_run();
                }

                if ui.button("Load").clicked() {
                    self.load_saved_run();
                }

                if let Some(run) = self.current_run.as_mut() {
                    ui.checkbox(&mut run.player_media_enabled, "Player media this run");
                }

                ui.separator();
                ui.small(format!("{} | Radiant Determinism", ENGINE_DISPLAY_NAME));
                ui.separator();
                ui.label(format!("Run: {}", self.setup.selected_datapack));
                ui.separator();
                if let Some(narrator) = self.narrator.as_ref() {
                    let status = narrator.status();
                    ui.label(format!(
                        "Narrator: {} ({})",
                        self.setup.gpu_narrator_model, self.setup.narrator_backend
                    ));
                    ui.small(format!("{} — {}", status.label, status.detail));
                } else {
                    ui.label("Narration: Off");
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ui_session.active_tab, GameTab::Game, "Game");
                ui.selectable_value(
                    &mut self.ui_session.active_tab,
                    GameTab::Inventory,
                    "Inventory",
                );
                ui.selectable_value(
                    &mut self.ui_session.active_tab,
                    GameTab::Character,
                    "Character",
                );
                ui.selectable_value(
                    &mut self.ui_session.active_tab,
                    GameTab::Diagnostics,
                    "Diagnostics",
                );
                ui.selectable_value(&mut self.ui_session.active_tab, GameTab::Library, "Library");
            });
        });

        let mut queued_command = None;

        if self.ui_session.active_tab == GameTab::Game {
            queued_command = show_game_action_bar(
                ctx,
                self.current_run.as_ref(),
                self.current_bundle.as_ref(),
                self.ui_session.media_focus_entity_id.as_deref(),
                &mut self.ui_session.current_input,
            );
        }

        match self.ui_session.active_tab {
            GameTab::Game => {
                let media_panel = build_media_panel_state_with_focus(
                    self.current_bundle.as_ref(),
                    self.current_run.as_ref(),
                    &self.diagnostic_events,
                    self.ui_session.media_focus_entity_id.as_deref(),
                );
                if queued_command.is_none() {
                    let (command, viewer_request, focus_entity_id) = show_game_tab(
                        ctx,
                        self.current_run.as_ref(),
                        self.current_bundle.as_ref(),
                        &media_panel,
                        self.setup.chaos_mode,
                        self.setup.fog_mode.as_str(),
                        &self.log_lines,
                    );
                    queued_command = command;
                    if viewer_request.is_some() {
                        self.ui_session.asset_viewer_request = viewer_request;
                    }
                    if focus_entity_id.is_some() {
                        self.ui_session.media_focus_entity_id = focus_entity_id;
                        ctx.request_repaint();
                    }
                } else {
                    let (_, viewer_request, focus_entity_id) = show_game_tab(
                        ctx,
                        self.current_run.as_ref(),
                        self.current_bundle.as_ref(),
                        &media_panel,
                        self.setup.chaos_mode,
                        self.setup.fog_mode.as_str(),
                        &self.log_lines,
                    );
                    if viewer_request.is_some() {
                        self.ui_session.asset_viewer_request = viewer_request;
                    }
                    if focus_entity_id.is_some() {
                        self.ui_session.media_focus_entity_id = focus_entity_id;
                        ctx.request_repaint();
                    }
                }
            }
            GameTab::Inventory => {
                if queued_command.is_none() {
                    let (command, viewer_request) = show_inventory_tab(
                        ctx,
                        self.current_run.as_ref(),
                        self.current_bundle.as_ref(),
                    );
                    queued_command = command;
                    if viewer_request.is_some() {
                        self.ui_session.asset_viewer_request = viewer_request;
                    }
                } else {
                    let (_, viewer_request) = show_inventory_tab(
                        ctx,
                        self.current_run.as_ref(),
                        self.current_bundle.as_ref(),
                    );
                    if viewer_request.is_some() {
                        self.ui_session.asset_viewer_request = viewer_request;
                    }
                }
            }
            GameTab::Character => {
                let (command, viewer_request) = show_character_tab(
                    ctx,
                    self.current_run.as_ref(),
                    self.current_bundle.as_ref(),
                );
                if queued_command.is_none() {
                    queued_command = command;
                }
                if viewer_request.is_some() {
                    self.ui_session.asset_viewer_request = viewer_request;
                }
            }
            GameTab::Diagnostics => {
                let save_path = current_save_path();
                let report = build_diagnostic_report(
                    &self.datapacks,
                    self.current_bundle.as_ref(),
                    self.current_run.as_ref(),
                    self.narrator.is_some(),
                    &save_path,
                    current_save_version(),
                    self.loaded_save_version,
                    datapack_schema_version(),
                    &self.diagnostic_events,
                );
                show_diagnostics_tab(ctx, &report)
            }
            GameTab::Library => show_library_tab(
                ctx,
                self.current_run
                    .as_ref()
                    .map(|run| run.datapack_id.as_str()),
                &mut self.ui_session.library,
            ),
        }

        if let Some(request) = self.ui_session.asset_viewer_request.as_ref()
            && !show_asset_viewer(ctx, request)
        {
            self.ui_session.asset_viewer_request = None;
        }

        self.show_narrator_item_proposal(ctx);
        self.show_narrator_status_proposal(ctx);
        self.show_narrator_npc_proposal(ctx);
        self.show_narrator_enemy_proposal(ctx);
        self.show_narrator_boss_proposal(ctx);
        self.show_narrator_objective_proposal(ctx);
        self.show_narrator_temporary_exit_proposal(ctx);
        self.show_narrator_ability_grant_proposal(ctx);

        if let Some(command) = queued_command {
            self.submit_command_text(command);
        }
    }

    fn selected_datapack_record(&self) -> Option<&DatapackRecord> {
        self.datapacks.valid.iter().find(|record| {
            record.folder_name == self.setup.selected_datapack
                || record.summary.display_name == self.setup.selected_datapack
        })
    }

    fn selected_invalid_datapack(&self) -> Option<&InvalidDatapack> {
        self.datapacks
            .invalid
            .iter()
            .find(|record| record.folder_name == self.setup.selected_datapack)
    }

    fn selected_datapack_label(&self) -> String {
        if let Some(record) = self.selected_datapack_record() {
            format!("{} [ready]", record.summary.display_name)
        } else if let Some(record) = self.selected_invalid_datapack() {
            format!(
                "{} [broken]",
                record
                    .display_name
                    .as_deref()
                    .unwrap_or(record.folder_name.as_str())
            )
        } else {
            self.setup.selected_datapack.clone()
        }
    }

    fn refresh_datapacks(&mut self) {
        let previous_selection = self.setup.selected_datapack.clone();
        self.datapacks = discover_datapacks();
        let still_present = self
            .datapacks
            .valid
            .iter()
            .any(|record| record.folder_name == previous_selection)
            || self
                .datapacks
                .invalid
                .iter()
                .any(|record| record.folder_name == previous_selection);

        if !still_present {
            self.setup.selected_datapack = self
                .datapacks
                .valid
                .first()
                .map(|record| record.folder_name.clone())
                .or_else(|| {
                    self.datapacks
                        .invalid
                        .first()
                        .map(|record| record.folder_name.clone())
                })
                .unwrap_or_else(|| "No cartridges found".to_owned());
        }

        self.log_lines.push(format!(
            "System: Cartridge bay refreshed: {} ready, {} broken.",
            self.datapacks.valid.len(),
            self.datapacks.invalid.len()
        ));
        self.persist_title_setup();
    }

    fn start_new_run(&mut self) {
        let Some(record) = self.selected_datapack_record() else {
            let (folder_name, reason) = if let Some(invalid) = self.selected_invalid_datapack() {
                (
                    invalid.folder_name.clone(),
                    invalid
                        .errors
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Selected cartridge failed validation.".to_owned()),
                )
            } else {
                (
                    self.setup.selected_datapack.clone(),
                    "Selected cartridge is not currently available.".to_owned(),
                )
            };
            self.diagnostic_events
                .push(GameEvent::SelectedPackFailedValidation {
                    folder_name: folder_name.clone(),
                    reason: reason.clone(),
                });
            self.current_bundle = None;
            self.current_run = None;
            self.narrator = None;
            self.log_lines.clear();
            self.log_lines.push(format!(
                "System: Selected cartridge '{}' failed validation and was not booted.",
                folder_name
            ));
            self.log_lines
                .push(format!("System: Validation error: {}", reason));
            return;
        };

        match load_datapack_bundle_by_folder(&record.folder_name) {
            Ok(bundle) => {
                if !bundle.rules.allowed_play_presets.is_empty()
                    && !bundle
                        .rules
                        .allowed_play_presets
                        .iter()
                        .any(|preset| preset == &self.setup.play_preset)
                {
                    self.log_lines.clear();
                    self.log_lines.push(format!(
                        "System: This cartridge permits {} play contracts; '{}' is unavailable.",
                        bundle.rules.allowed_play_presets.join(", "),
                        self.setup.play_preset
                    ));
                    return;
                }
                let GeneratedRun {
                    mut state,
                    mut log_lines,
                } = generate_new_run(&bundle);
                if self.setup.permits_generated_persistent_content() {
                    match load_player_templates_for_datapack(&state.datapack_id) {
                        Ok(templates) => {
                            let templates = enabled_player_templates(templates);
                            let item_templates: Vec<_> = templates
                                .iter()
                                .filter(|template| template.kind.is_item_like())
                                .cloned()
                                .collect();
                            let item_count = item_templates.len();
                            state
                                .inventory
                                .extend(item_templates.into_iter().map(|item| InventoryEntry {
                                    instance_id: format!("player_template_{}", item.id),
                                    template_id: item.id.clone(),
                                    id: item.id,
                                    bucket: ItemBucket::Inventory,
                                    location_id: None,
                                    lifecycle: InstanceLifecycle::Active,
                                    name: item.name,
                                    description: item.description,
                                    damage: item.damage,
                                    origin: ItemOrigin::NarratorApproved,
                                    condition: item.condition,
                                    tags: item.tags,
                                    actor: item.actor,
                                    disposition: item.disposition,
                                    inventory: Vec::new(),
                                    equipped_item_id: None,
                                }));
                            let mut npc_count = 0;
                            for template in templates
                                .iter()
                                .filter(|template| template.kind == PlayerTemplateKind::Npc)
                            {
                                let PlayerTemplateMechanics::Npc { faction } =
                                    template.effective_mechanics()
                                else {
                                    continue;
                                };
                                let npc = NpcInstance {
                                    instance_id: format!("player_npc_{}", template.id),
                                    template_id: template.id.clone(),
                                    location_id: state.current_location_id.clone(),
                                    lifecycle: InstanceLifecycle::Active,
                                    provenance: "player_library".to_owned(),
                                    name: template.name.clone(),
                                    description: template.description.clone(),
                                    faction,
                                    tags: template.tags.clone(),
                                    disposition: template.disposition.clone(),
                                    actor: template.actor.clone().unwrap_or_default(),
                                    quest_objective_ids: Vec::new(),
                                    inventory: Vec::new(),
                                    equipped_item_id: None,
                                };
                                state
                                    .location_npcs
                                    .entry(npc.location_id.clone())
                                    .or_default()
                                    .push(npc);
                                npc_count += 1;
                            }
                            let effects = templates
                                .iter()
                                .filter(|template| {
                                    matches!(
                                        template.kind,
                                        PlayerTemplateKind::Status | PlayerTemplateKind::Effect
                                    )
                                })
                                .filter_map(|template| match template.effective_mechanics() {
                                    PlayerTemplateMechanics::Status {
                                        trigger,
                                        duration,
                                        additive_damage,
                                        additive_healing,
                                        required_target_tags,
                                        required_context_tags,
                                    } => Some(StatusEffectInstance {
                                        instance_id: format!("player_effect_{}", template.id),
                                        template_id: template.id.clone(),
                                        owner: StatusOwner::Entity,
                                        owner_id: Some(state.player_entity_id.clone()),
                                        trigger: effect_trigger_from_template(&trigger),
                                        duration: effect_duration_from_template(&duration),
                                        additive_damage,
                                        additive_healing,
                                        timing_cost_delta: 0,
                                        timing_action_kinds: Vec::new(),
                                        blocked_action_kinds: Vec::new(),
                                        required_target_tags,
                                        required_context_tags,
                                        lifecycle: InstanceLifecycle::Active,
                                    }),
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            let effect_count = effects.len();
                            state.status_effects.extend(effects);
                            let mut enemy_count = 0;
                            for template in templates
                                .iter()
                                .filter(|template| template.kind == PlayerTemplateKind::Enemy)
                            {
                                let PlayerTemplateMechanics::Enemy { max_hp, damage } =
                                    template.effective_mechanics()
                                else {
                                    continue;
                                };
                                let enemy_id = format!("player_enemy_{}", template.id);
                                state.enemies_alive.insert(enemy_id.clone());
                                state.enemy_hp.insert(enemy_id.clone(), max_hp);
                                state
                                    .location_enemies
                                    .entry(state.current_location_id.clone())
                                    .or_default()
                                    .push(enemy_id.clone());
                                state.enemy_instances.insert(
                                    enemy_id.clone(),
                                    EncounterInstance {
                                        instance_id: enemy_id.clone(),
                                        template_id: template.id.clone(),
                                        location_id: state.current_location_id.clone(),
                                        bucket: EncounterBucket::Live,
                                        lifecycle: InstanceLifecycle::Active,
                                        current_hp: max_hp,
                                        max_hp,
                                        provenance: "player_library".to_owned(),
                                        name: template.name.clone(),
                                        description: template.description.clone(),
                                        damage,
                                        traversal_cost: 1,
                                        attack_cost: 1,
                                        tags: template.tags.clone(),
                                        capabilities: vec![
                                            "health".to_owned(),
                                            "combat".to_owned(),
                                        ],
                                    },
                                );
                                enemy_count += 1;
                            }
                            let mut boss_count = 0;
                            for template in templates
                                .iter()
                                .filter(|template| template.kind == PlayerTemplateKind::Boss)
                            {
                                let PlayerTemplateMechanics::Boss { max_hp, damage } =
                                    template.effective_mechanics()
                                else {
                                    continue;
                                };
                                let boss_id = format!("player_boss_{}", template.id);
                                state.bosses_alive.insert(boss_id.clone());
                                state.boss_hp.insert(boss_id.clone(), max_hp);
                                state
                                    .location_bosses
                                    .entry(state.current_location_id.clone())
                                    .or_default()
                                    .push(boss_id.clone());
                                state.boss_instances.insert(
                                    boss_id.clone(),
                                    EncounterInstance {
                                        instance_id: boss_id.clone(),
                                        template_id: template.id.clone(),
                                        location_id: state.current_location_id.clone(),
                                        bucket: EncounterBucket::Live,
                                        lifecycle: InstanceLifecycle::Active,
                                        current_hp: max_hp,
                                        max_hp,
                                        provenance: "player_library".to_owned(),
                                        name: template.name.clone(),
                                        description: template.description.clone(),
                                        damage,
                                        traversal_cost: 1,
                                        attack_cost: 1,
                                        tags: template.tags.clone(),
                                        capabilities: vec![
                                            "health".to_owned(),
                                            "combat".to_owned(),
                                        ],
                                    },
                                );
                                boss_count += 1;
                            }
                            let objectives = templates
                                .iter()
                                .filter(|template| template.kind == PlayerTemplateKind::Objective)
                                .filter_map(|template| match template.effective_mechanics() {
                                    PlayerTemplateMechanics::Objective { completion_key } => {
                                        Some(ObjectiveState {
                                            instance_id: format!(
                                                "player_objective_{}",
                                                template.id
                                            ),
                                            template_id: template.id.clone(),
                                            lifecycle: InstanceLifecycle::Active,
                                            bucket: ObjectiveBucket::Active,
                                            id: template.id.clone(),
                                            name: template.name.clone(),
                                            description: template.description.clone(),
                                            target_boss_id: None,
                                            required_item_id: None,
                                            required_location_id: Some(completion_key),
                                            completed: false,
                                        })
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            let objective_count = objectives.len();
                            state.additional_objectives.extend(objectives);
                            let mut known_location_count = 0;
                            for template in templates
                                .iter()
                                .filter(|template| template.kind == PlayerTemplateKind::Location)
                            {
                                let PlayerTemplateMechanics::Location {
                                    location_id,
                                    initially_known,
                                } = template.effective_mechanics()
                                else {
                                    continue;
                                };
                                if initially_known
                                    && bundle
                                        .locations
                                        .iter()
                                        .any(|location| location.id == location_id)
                                    && state.known_locations.insert(location_id)
                                {
                                    known_location_count += 1;
                                }
                            }
                            let exits = templates
                                .iter()
                                .filter(|template| {
                                    template.kind == PlayerTemplateKind::TemporaryExit
                                })
                                .filter_map(|template| match template.effective_mechanics() {
                                    PlayerTemplateMechanics::TemporaryExit {
                                        from_location_id,
                                        to_location_id,
                                        locked,
                                    } => Some(TemporaryExitInstance {
                                        instance_id: format!("player_exit_{}", template.id),
                                        template_id: template.id.clone(),
                                        from_location_id,
                                        to_location_id,
                                        locked,
                                        lifecycle: InstanceLifecycle::Active,
                                        provenance: "player_library".to_owned(),
                                    }),
                                    _ => None,
                                })
                                .collect::<Vec<_>>();
                            let exit_count = exits.len();
                            state.temporary_exits.extend(exits);
                            if item_count > 0
                                || npc_count > 0
                                || effect_count > 0
                                || enemy_count > 0
                                || boss_count > 0
                                || objective_count > 0
                                || known_location_count > 0
                                || exit_count > 0
                            {
                                log_lines.push(format!(
                                    "Personal collection restored: {} item(s), {} NPC(s), {} effect(s), {} enemy(ies), {} boss(es), {} objective(s), {} known location(s), and {} temporary exit(s) joined this run.",
                                    item_count, npc_count, effect_count, enemy_count, boss_count, objective_count, known_location_count, exit_count
                                ));
                            }
                        }
                        Err(error) => log_lines.push(format!("System: {error}")),
                    }
                }
                let pack_uses_narration = bundle.rules.narration.usage != "unused";
                let mock_narrator =
                    pack_uses_narration.then(|| MockNarrator::new(&bundle, self.setup.chaos_mode));
                let narrator = (pack_uses_narration && self.setup.narrator_backend != "None")
                    .then(|| build_narrator(&bundle, &self.setup));
                let narrated_start = if self
                    .setup
                    .narration_preferences
                    .allows(crate::game::narrator::NarrationHook::Summary)
                {
                    match narration_route(&bundle, crate::game::narrator::NarrationHook::Summary) {
                        NarrationRoute::SelectedNarrator => narrator
                            .as_ref()
                            .map(|narrator| narrator.narrate_run_start(&bundle, &state, &log_lines))
                            .unwrap_or_default(),
                        NarrationRoute::Mock => mock_narrator
                            .as_ref()
                            .map(|narrator| narrator.narrate_run_start(&bundle, &state, &log_lines))
                            .unwrap_or_default(),
                        NarrationRoute::None => Vec::new(),
                    }
                } else {
                    Vec::new()
                };
                let narrator_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_item_proposals())
                    .unwrap_or_default();
                let narrator_status_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_status_proposals())
                    .unwrap_or_default();
                let narrator_npc_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_npc_proposals())
                    .unwrap_or_default();
                let narrator_enemy_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_enemy_proposals())
                    .unwrap_or_default();
                let narrator_boss_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_boss_proposals())
                    .unwrap_or_default();
                let narrator_objective_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_objective_proposals())
                    .unwrap_or_default();
                let narrator_temporary_exit_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_temporary_exit_proposals())
                    .unwrap_or_default();
                let narrator_ability_grant_proposals = narrator
                    .as_ref()
                    .map(|n| n.take_ability_grant_proposals())
                    .unwrap_or_default();
                let narrator_proposal_diagnostics = narrator
                    .as_ref()
                    .map(|n| n.take_proposal_diagnostics())
                    .unwrap_or_default();
                self.current_bundle = Some(bundle);
                self.current_run = Some(state.clone());
                self.narrator = narrator;
                self.mock_narrator = mock_narrator;
                self.screen = AppScreen::ActiveRun;
                self.ui_session.active_tab = GameTab::Game;
                self.ui_session.current_input.clear();
                self.ui_session.asset_viewer_request = None;
                self.log_lines.clear();
                self.diagnostic_events.clear();
                self.loaded_save_version = Some(current_save_version());
                self.pending_narrator_item = None;
                self.queued_narrator_items.clear();
                self.pending_narrator_status = None;
                self.queued_narrator_statuses.clear();
                self.pending_narrator_npc = None;
                self.queued_narrator_npcs.clear();
                self.pending_narrator_enemy = None;
                self.queued_narrator_enemies.clear();
                self.pending_narrator_boss = None;
                self.queued_narrator_bosses.clear();
                self.pending_narrator_objective = None;
                self.queued_narrator_objectives.clear();
                self.pending_narrator_temporary_exit = None;
                self.queued_narrator_temporary_exits.clear();
                self.pending_narrator_ability_grant = None;
                self.queued_narrator_ability_grants.clear();
                self.enqueue_narrator_item_proposals(narrator_proposals);
                self.enqueue_narrator_status_proposals(narrator_status_proposals);
                self.enqueue_narrator_npc_proposals(narrator_npc_proposals);
                self.enqueue_narrator_enemy_proposals(narrator_enemy_proposals);
                self.enqueue_narrator_boss_proposals(narrator_boss_proposals);
                self.enqueue_narrator_objective_proposals(narrator_objective_proposals);
                self.enqueue_narrator_temporary_exit_proposals(narrator_temporary_exit_proposals);
                self.enqueue_narrator_ability_grant_proposals(narrator_ability_grant_proposals);
                self.log_lines.extend(narrated_start);
                self.log_lines.extend(narrator_proposal_diagnostics);
                self.log_lines.push(format!(
                    "System: {} setup dialed in: difficulty {:.0}%, chaos {:.0}%, capsule {}.",
                    ENGINE_DISPLAY_NAME,
                    self.setup.difficulty * 100.0,
                    self.setup.chaos_mode * 100.0,
                    self.setup.dm_capsule
                ));
                self.log_lines.push(format!(
                    "System: Map visibility mode set to {}.",
                    self.setup.fog_mode
                ));
                self.log_lines.push(format!(
                    "System: {} created run state for {}.",
                    ENGINE_DISPLAY_NAME, state.datapack_display_name
                ));
            }
            Err(errors) => {
                self.narrator = None;
                self.log_lines.clear();
                self.log_lines.push(
                    "System: Failed to generate a run from the selected datapack.".to_owned(),
                );
                for error in errors {
                    self.log_lines
                        .push(format!("System: Validation error: {}", error));
                }
            }
        }
    }

    fn submit_current_input(&mut self) {
        let input = self.ui_session.current_input.trim().to_owned();
        if input.is_empty() {
            return;
        }

        if self.narration_task.is_some() {
            self.log_lines.push(
                "Narrator is still composing the previous turn. Please wait for it to finish."
                    .to_owned(),
            );
            return;
        }

        self.log_lines.push(format!("> {}", input));
        self.ui_session.current_input.clear();

        let Some(bundle) = self.current_bundle.as_ref() else {
            self.log_lines
                .push("No active datapack bundle is loaded.".to_owned());
            return;
        };
        let Some(run) = self.current_run.as_mut() else {
            self.log_lines.push("No active run exists yet.".to_owned());
            return;
        };

        let action = match crate::game::actions::parse_command(&input) {
            Ok(parsed_command) => parsed_command.action,
            Err(reason) => {
                // Named actor instructions can start with a reducer verb ("attack with Lira").
                // Resolve that bounded host intent before rejecting the phrase as a malformed
                // player-only command.
                let actor_attack_intent =
                    crate::game::combat::resolve_actor_attack_intent(run, &input);
                let social_intent = run.resolve_social_intent(&input);
                let actor_ability_intent =
                    crate::game::resolve_actor_ability_intent(run, bundle, &input);
                let scroll_learning_intent =
                    crate::game::resolve_controlled_scroll_learning_intent(run, &input);
                let ability_intent =
                    crate::game::resolve_controlled_ability_intent(run, bundle, &input);
                let verb = input
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let reducer_verb = matches!(
                    verb.as_str(),
                    "help"
                        | "look"
                        | "go"
                        | "unlock"
                        | "barricade"
                        | "inspect"
                        | "take"
                        | "equip"
                        | "use"
                        | "attack"
                        | "wait"
                );
                if reducer_verb && actor_attack_intent.is_none() && social_intent.is_none() {
                    self.log_lines.push(format!("{reason} Try: help, look, go <location>, unlock <location>, barricade <location>, inspect <thing>, take <item>, equip <item>, use <item>, attack, or wait. A named combat-capable actor can attack a target."));
                    return;
                }
                if let Some(transfer_intent) = run.resolve_player_drop_to_location(&input) {
                    match transfer_intent {
                        Ok((item_id, origin, destination)) => {
                            crate::game::GameAction::TransferItem {
                                item_id,
                                origin,
                                destination,
                                intent: crate::game::TransferIntent::Ordinary,
                            }
                        }
                        Err(reason) => {
                            self.log_lines.push(reason);
                            return;
                        }
                    }
                } else if let Some(transfer_intent) = run.resolve_player_transfer_to_actor(&input) {
                    match transfer_intent {
                        Ok((item_id, origin, destination)) => {
                            crate::game::GameAction::TransferItem {
                                item_id,
                                origin,
                                destination,
                                intent: crate::game::TransferIntent::Ordinary,
                            }
                        }
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Actor transfer unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(transfer_intent) = run.resolve_actor_transfer_to_player(&input) {
                    match transfer_intent {
                        Ok((item_id, origin, destination)) => {
                            crate::game::GameAction::TransferItem {
                                item_id,
                                origin,
                                destination,
                                intent: crate::game::TransferIntent::Ordinary,
                            }
                        }
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Actor transfer unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(theft_intent) = run.resolve_player_theft_from_actor(&input) {
                    match theft_intent {
                        Ok((item_id, origin, destination)) => {
                            crate::game::GameAction::TransferItem {
                                item_id,
                                origin,
                                destination,
                                intent: crate::game::TransferIntent::Theft,
                            }
                        }
                        Err(reason) => {
                            self.log_lines.push(format!("Theft unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(actor_ability_intent) = actor_ability_intent {
                    match actor_ability_intent {
                        Ok((origin_id, grant_id, target)) => crate::game::GameAction::UseAbility {
                            origin_id,
                            grant_id,
                            target,
                        },
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Actor ability unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(scroll_learning_intent) = scroll_learning_intent {
                    match scroll_learning_intent {
                        Ok((item_id, grant_id)) => crate::game::GameAction::LearnAbilityFromItem {
                            learner_id: run.player_entity_id.clone(),
                            item_id,
                            grant_id,
                        },
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Source study unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(ability_intent) = ability_intent {
                    match ability_intent {
                        Ok(crate::game::AbilityIntent::Use { grant_id, target }) => {
                            crate::game::GameAction::UseAbility {
                                origin_id: run.player_entity_id.clone(),
                                grant_id,
                                target,
                            }
                        }
                        Ok(crate::game::AbilityIntent::Unlock { template_id }) => {
                            crate::game::GameAction::UnlockAbility {
                                owner: run.controlled_inventory_owner(),
                                template_id,
                            }
                        }
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Ability unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(social_intent) = social_intent {
                    match social_intent {
                        Ok((subject_id, kind)) => crate::game::GameAction::ApplySocialEvent {
                            origin_id: run.player_entity_id.clone(),
                            subject_id,
                            kind,
                        },
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Social action unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(actor_intent) = actor_attack_intent {
                    match actor_intent {
                        Ok((origin_id, target)) => {
                            crate::game::GameAction::ActorAttack { origin_id, target }
                        }
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Actor action unavailable: {reason}"));
                            return;
                        }
                    }
                } else if let Some(actor_intent) =
                    crate::game::resolve_actor_move_intent(run, bundle, &input)
                {
                    match actor_intent {
                        Ok((origin_id, destination)) => crate::game::GameAction::ActorMove {
                            origin_id,
                            destination,
                        },
                        Err(reason) => {
                            self.log_lines
                                .push(format!("Actor movement unavailable: {reason}"));
                            return;
                        }
                    }
                } else {
                    if let Some(narrator) = self.narrator.as_ref() {
                        let decision = crate::game::narrator::decide_free_text_proposals(
                            &self.setup.play_contract(),
                            self.setup.chaos_mode,
                            run.turn_index,
                            &input,
                        );
                        let allowed_kinds = self.setup.authorized_proposal_kinds();
                        let selected_kind = decision
                            .permitted
                            .then(|| {
                                crate::game::narrator::select_host_proposal_kind(
                                    &allowed_kinds,
                                    run.turn_index,
                                    &input,
                                )
                            })
                            .flatten();
                        let lines = narrator.narrate_player_message(
                            bundle,
                            run,
                            &input,
                            selected_kind.as_deref(),
                        );
                        let proposals = narrator.take_item_proposals();
                        let statuses = narrator.take_status_proposals();
                        let npcs = narrator.take_npc_proposals();
                        let enemies = narrator.take_enemy_proposals();
                        let bosses = narrator.take_boss_proposals();
                        let objectives = narrator.take_objective_proposals();
                        let exits = narrator.take_temporary_exit_proposals();
                        let ability_grants = narrator.take_ability_grant_proposals();
                        let proposal_diagnostics = narrator.take_proposal_diagnostics();
                        let proposal_count = proposals.len()
                            + statuses.len()
                            + npcs.len()
                            + enemies.len()
                            + bosses.len()
                            + objectives.len()
                            + exits.len()
                            + ability_grants.len();
                        if decision.permitted && proposal_count > 0 {
                            self.log_lines.push(format!(
                            "Narrator: proposal generated and admitted to the editable approval card (host gate {}%).",
                            decision.probability_percent
                        ));
                        } else if decision.permitted && allowed_kinds.is_empty() {
                            self.log_lines
                                .push("Narrator: no compatible template available.".to_owned());
                        } else if decision.permitted {
                            self.log_lines.push(format!(
                                "Narrator: required proposal was not returned (host gate {}%).",
                                decision.probability_percent
                            ));
                        } else if !decision.eligible {
                            self.log_lines.push(
                            "Narrator: narration only (host found no proposal-worthy free-text intent)."
                                .to_owned(),
                        );
                        } else {
                            self.log_lines.push(format!(
                            "Narrator: narration only (host proposal gate {}% declined this turn).",
                            decision.probability_percent
                        ));
                        }
                        self.log_lines.extend(proposal_diagnostics);
                        self.enqueue_narrator_item_proposals(proposals);
                        self.enqueue_narrator_status_proposals(statuses);
                        self.enqueue_narrator_npc_proposals(npcs);
                        self.enqueue_narrator_enemy_proposals(enemies);
                        self.enqueue_narrator_boss_proposals(bosses);
                        self.enqueue_narrator_objective_proposals(objectives);
                        self.enqueue_narrator_temporary_exit_proposals(exits);
                        self.enqueue_narrator_ability_grant_proposals(ability_grants);
                        self.log_lines
                            .extend(lines.into_iter().map(|line| format!("Narrator: {line}")));
                    } else {
                        self.log_lines.push("Narration is off. Use a reducer command such as help, look, go, inspect, take, or attack.".to_owned());
                    }
                    return;
                }
            }
        };

        let raw_outcome = apply_action(run, bundle, action.clone());
        self.diagnostic_events
            .extend(raw_outcome.events.iter().cloned());
        let hook = narration_hook_for_action(&action, &raw_outcome);
        let route = if self.setup.narration_preferences.allows(hook) {
            narration_route(bundle, hook)
        } else {
            NarrationRoute::None
        };
        let ambient_decision = crate::game::narrator::decide_ambient_proposal(
            &self.setup.play_contract(),
            self.setup.chaos_mode,
            run.turn_index,
            run.last_ambient_proposal_turn,
        );
        let ambient_allowed_kinds = self.setup.authorized_proposal_kinds();
        let ambient_kind = ambient_decision
            .permitted
            .then(|| {
                crate::game::narrator::select_host_proposal_kind(
                    &ambient_allowed_kinds,
                    run.turn_index,
                    "ambient discovery",
                )
            })
            .flatten();
        if matches!(route, NarrationRoute::SelectedNarrator)
            && let Some(narrator) = self.narrator.take()
        {
            if ambient_kind.is_some() {
                run.last_ambient_proposal_turn = Some(run.turn_index);
            }
            let task_bundle = bundle.clone();
            let task_action = action.clone();
            let task_outcome = raw_outcome.clone();
            let task_state = run.clone();
            let task_kind = ambient_kind.clone();
            let task_traces = action_traces(&action, &raw_outcome)
                .into_iter()
                .map(|trace| trace.chat_line())
                .collect::<Vec<_>>();
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let lines = narrator.narrate_action(
                    &task_bundle,
                    &task_action,
                    &task_outcome,
                    &task_state,
                    task_kind.as_deref(),
                );
                // The proposal queues belong to the live adapter.  Drain them in the same worker
                // immediately after the request, then return both the queues and adapter intact.
                let _ = sender.send(NarrationTaskResult {
                    item_proposals: narrator.take_item_proposals(),
                    status_proposals: narrator.take_status_proposals(),
                    npc_proposals: narrator.take_npc_proposals(),
                    enemy_proposals: narrator.take_enemy_proposals(),
                    boss_proposals: narrator.take_boss_proposals(),
                    objective_proposals: narrator.take_objective_proposals(),
                    temporary_exit_proposals: narrator.take_temporary_exit_proposals(),
                    ability_grant_proposals: narrator.take_ability_grant_proposals(),
                    diagnostics: narrator.take_proposal_diagnostics(),
                    narrator,
                    lines,
                    action_traces: task_traces,
                });
            });
            self.narration_task = Some(receiver);
            self.log_lines
                .push("Narrator: composing reducer-confirmed presentation…".to_owned());
            return;
        }
        let narrated_lines = match route {
            NarrationRoute::None => raw_outcome.lines.clone(),
            NarrationRoute::Mock => self
                .mock_narrator
                .as_ref()
                .map(|narrator| narrator.narrate_action(bundle, &action, &raw_outcome, run, None))
                .unwrap_or_else(|| raw_outcome.lines.clone()),
            NarrationRoute::SelectedNarrator => {
                if ambient_kind.is_some() {
                    run.last_ambient_proposal_turn = Some(run.turn_index);
                }
                self.narrator
                    .as_ref()
                    .map(|narrator| {
                        narrator.narrate_action(
                            bundle,
                            &action,
                            &raw_outcome,
                            run,
                            ambient_kind.as_deref(),
                        )
                    })
                    .unwrap_or_else(|| raw_outcome.lines.clone())
            }
        };
        let narrator_proposals = self
            .narrator
            .as_ref()
            .map(|narrator| narrator.take_item_proposals())
            .unwrap_or_default();
        let narrator_status_proposals = self
            .narrator
            .as_ref()
            .map(|narrator| narrator.take_status_proposals())
            .unwrap_or_default();
        let narrator_npc_proposals = self
            .narrator
            .as_ref()
            .map(|n| n.take_npc_proposals())
            .unwrap_or_default();
        let narrator_enemy_proposals = self
            .narrator
            .as_ref()
            .map(|n| n.take_enemy_proposals())
            .unwrap_or_default();
        let narrator_boss_proposals = self
            .narrator
            .as_ref()
            .map(|n| n.take_boss_proposals())
            .unwrap_or_default();
        let narrator_objective_proposals = self
            .narrator
            .as_ref()
            .map(|n| n.take_objective_proposals())
            .unwrap_or_default();
        let narrator_temporary_exit_proposals = self
            .narrator
            .as_ref()
            .map(|n| n.take_temporary_exit_proposals())
            .unwrap_or_default();
        let narrator_ability_grant_proposals = self
            .narrator
            .as_ref()
            .map(|n| n.take_ability_grant_proposals())
            .unwrap_or_default();
        let narrator_proposal_diagnostics = self
            .narrator
            .as_ref()
            .map(|n| n.take_proposal_diagnostics())
            .unwrap_or_default();
        self.enqueue_narrator_item_proposals(narrator_proposals);
        self.enqueue_narrator_status_proposals(narrator_status_proposals);
        self.enqueue_narrator_npc_proposals(narrator_npc_proposals);
        self.enqueue_narrator_enemy_proposals(narrator_enemy_proposals);
        self.enqueue_narrator_boss_proposals(narrator_boss_proposals);
        self.enqueue_narrator_objective_proposals(narrator_objective_proposals);
        self.enqueue_narrator_temporary_exit_proposals(narrator_temporary_exit_proposals);
        self.enqueue_narrator_ability_grant_proposals(narrator_ability_grant_proposals);
        if let Some(kind) = ambient_kind {
            self.log_lines.push(format!(
                "Narrator: rare ambient discovery proposal requested ({kind}; host chance {}% after {} ticks).",
                ambient_decision.probability_percent, ambient_decision.cooldown_turns
            ));
        }
        if matches!(route, NarrationRoute::None) {
            self.log_lines.extend(narrated_lines);
        } else {
            self.log_lines.extend(
                narrated_lines
                    .into_iter()
                    .map(|line| format!("Narrator: {line}")),
            );
            // The chat gets a generic, reducer-owned account for every traceable event. This
            // makes origin, impact, action type, and result visible even if a small narrator
            // produces terse prose or no prose at all.
            self.log_lines.extend(
                action_traces(&action, &raw_outcome)
                    .into_iter()
                    .map(|trace| trace.chat_line()),
            );
        }
        self.log_lines.extend(narrator_proposal_diagnostics);
    }

    fn enqueue_narrator_item_proposals(&mut self, proposals: Vec<ValidatedItemProposal>) {
        if !self.setup.permits_generated_persistent_content() {
            return;
        }
        self.queued_narrator_items
            .extend(proposals.into_iter().map(|proposal| NarratorItemProposal {
                name: proposal.name,
                description: proposal.description,
                damage: proposal.damage,
                condition: None,
                tags: proposal.tags,
                actor: ProposalActorConfiguration::from_narrator(proposal.actor),
            }));
        if self.pending_narrator_item.is_none() && !self.queued_narrator_items.is_empty() {
            self.pending_narrator_item = Some(self.queued_narrator_items.remove(0));
        }
    }

    fn enqueue_narrator_status_proposals(&mut self, proposals: Vec<ValidatedStatusProposal>) {
        if !self.setup.permits_generated_persistent_content()
            || !self
                .setup
                .play_contract()
                .allow_generated_mechanical_effects
        {
            return;
        }
        self.queued_narrator_statuses
            .extend(
                proposals
                    .into_iter()
                    .map(|proposal| NarratorStatusProposal {
                        name: proposal.name,
                        trigger: proposal.trigger,
                        duration: proposal.duration,
                        additive_damage: proposal.additive_damage,
                        additive_healing: proposal.additive_healing,
                        required_target_tags: proposal.required_target_tags,
                        required_context_tags: proposal.required_context_tags,
                    }),
            );
        if self.pending_narrator_status.is_none() && !self.queued_narrator_statuses.is_empty() {
            self.pending_narrator_status = Some(self.queued_narrator_statuses.remove(0));
        }
    }

    fn enqueue_narrator_npc_proposals(&mut self, proposals: Vec<ValidatedNpcProposal>) {
        if !self.setup.permits_generated_persistent_content() {
            return;
        }
        self.queued_narrator_npcs
            .extend(proposals.into_iter().map(|p| NarratorNpcProposal {
                name: p.name,
                description: p.description,
                faction: p.faction,
                tags: p.tags,
                actor: ProposalActorConfiguration::from_narrator(p.actor),
            }));
        if self.pending_narrator_npc.is_none() && !self.queued_narrator_npcs.is_empty() {
            self.pending_narrator_npc = Some(self.queued_narrator_npcs.remove(0));
        }
    }

    fn enqueue_narrator_enemy_proposals(&mut self, proposals: Vec<ValidatedEnemyProposal>) {
        if !self.setup.permits_generated_persistent_content()
            || !self
                .setup
                .play_contract()
                .allow_generated_mechanical_effects
        {
            return;
        }
        self.queued_narrator_enemies
            .extend(proposals.into_iter().map(|p| NarratorEnemyProposal {
                name: p.name,
                description: p.description,
                max_hp: p.max_hp,
                damage: p.damage,
                tags: p.tags,
            }));
        if self.pending_narrator_enemy.is_none() && !self.queued_narrator_enemies.is_empty() {
            self.pending_narrator_enemy = Some(self.queued_narrator_enemies.remove(0));
        }
    }

    fn enqueue_narrator_boss_proposals(&mut self, proposals: Vec<ValidatedBossProposal>) {
        if !self.setup.permits_generated_persistent_content()
            || !self
                .setup
                .play_contract()
                .allow_generated_mechanical_effects
        {
            return;
        }
        self.queued_narrator_bosses
            .extend(proposals.into_iter().map(|p| NarratorBossProposal {
                name: p.name,
                description: p.description,
                max_hp: p.max_hp,
                damage: p.damage,
                tags: p.tags,
            }));
        if self.pending_narrator_boss.is_none() && !self.queued_narrator_bosses.is_empty() {
            self.pending_narrator_boss = Some(self.queued_narrator_bosses.remove(0));
        }
    }

    fn enqueue_narrator_objective_proposals(&mut self, proposals: Vec<ValidatedObjectiveProposal>) {
        if !self.setup.permits_generated_persistent_content() {
            return;
        }
        self.queued_narrator_objectives
            .extend(proposals.into_iter().map(|p| NarratorObjectiveProposal {
                name: p.name,
                description: p.description,
                completion_key: p.completion_key,
            }));
        if self.pending_narrator_objective.is_none() && !self.queued_narrator_objectives.is_empty()
        {
            self.pending_narrator_objective = Some(self.queued_narrator_objectives.remove(0));
        }
    }

    fn enqueue_narrator_temporary_exit_proposals(
        &mut self,
        proposals: Vec<ValidatedTemporaryExitProposal>,
    ) {
        if !self.setup.permits_generated_persistent_content() {
            return;
        }
        self.queued_narrator_temporary_exits
            .extend(
                proposals
                    .into_iter()
                    .map(|p| NarratorTemporaryExitProposal {
                        name: p.name,
                        description: p.description,
                        from_location_id: p.from_location_id,
                        to_location_id: p.to_location_id,
                        locked: p.locked,
                    }),
            );
        if self.pending_narrator_temporary_exit.is_none()
            && !self.queued_narrator_temporary_exits.is_empty()
        {
            self.pending_narrator_temporary_exit =
                Some(self.queued_narrator_temporary_exits.remove(0));
        }
    }

    fn enqueue_narrator_ability_grant_proposals(
        &mut self,
        proposals: Vec<ValidatedAbilityGrantProposal>,
    ) {
        if !self.setup.permits_generated_persistent_content()
            || !self
                .setup
                .play_contract()
                .allow_generated_mechanical_effects
        {
            return;
        }
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        for proposal in proposals {
            if bundle
                .abilities
                .iter()
                .any(|ability| ability.id == proposal.template_id)
            {
                self.queued_narrator_ability_grants
                    .push(NarratorAbilityGrantProposal {
                        template_id: proposal.template_id,
                        remaining_uses: proposal.remaining_uses,
                        duration_turns: proposal.duration_turns,
                        recipient_id: "player".to_owned(),
                    });
            } else {
                self.log_lines.push(
                    "Narrator ability grant proposal rejected: template is not host-known."
                        .to_owned(),
                );
            }
        }
        if self.pending_narrator_ability_grant.is_none()
            && !self.queued_narrator_ability_grants.is_empty()
        {
            self.pending_narrator_ability_grant =
                Some(self.queued_narrator_ability_grants.remove(0));
        }
    }

    fn submit_command_text(&mut self, command: String) {
        self.ui_session.current_input = command;
        self.submit_current_input();
    }

    fn poll_narration_task(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.narration_task.as_ref() else {
            return;
        };
        let result = receiver.try_recv();
        match result {
            Ok(result) => {
                self.narration_task = None;
                self.narrator = Some(result.narrator);
                self.enqueue_narrator_item_proposals(result.item_proposals);
                self.enqueue_narrator_status_proposals(result.status_proposals);
                self.enqueue_narrator_npc_proposals(result.npc_proposals);
                self.enqueue_narrator_enemy_proposals(result.enemy_proposals);
                self.enqueue_narrator_boss_proposals(result.boss_proposals);
                self.enqueue_narrator_objective_proposals(result.objective_proposals);
                self.enqueue_narrator_temporary_exit_proposals(result.temporary_exit_proposals);
                self.enqueue_narrator_ability_grant_proposals(result.ability_grant_proposals);
                self.log_lines.extend(
                    result
                        .lines
                        .into_iter()
                        .map(|line| format!("Narrator: {line}")),
                );
                self.log_lines.extend(result.action_traces);
                self.log_lines.extend(result.diagnostics);
                ctx.request_repaint();
            }
            Err(mpsc::TryRecvError::Empty) => {
                // eframe otherwise sleeps after the current frame; keep polling without making
                // the UI thread wait on the narrator transport.
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.narration_task = None;
                self.log_lines.push(
                    "Narrator worker stopped before returning a response. Choose Mock or retry a later turn."
                        .to_owned(),
                );
            }
        }
    }

    fn show_narrator_ability_grant_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_ability_grant.clone() else {
            return;
        };
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        if !bundle
            .abilities
            .iter()
            .any(|ability| ability.id == proposal.template_id)
        {
            proposal.template_id = bundle
                .abilities
                .first()
                .map(|ability| ability.id.clone())
                .unwrap_or_default();
        }
        let recipients = bundle
            .abilities
            .iter()
            .find(|ability| ability.id == proposal.template_id)
            .map(|ability| {
                let mut recipients = Vec::new();
                if crate::game::actor_satisfies_ability_access(run, ability, &run.player_entity_id)
                {
                    let label = run
                        .runtime_actor(&run.player_entity_id)
                        .map(|actor| format!("{} (controlled)", actor.name))
                        .unwrap_or_else(|| "Controlled actor".to_owned());
                    recipients.push((run.player_entity_id.clone(), label));
                }
                recipients.extend(
                    run.runtime_actor_ids_at(&run.current_location_id)
                        .into_iter()
                        .filter(|id| id != &run.player_entity_id)
                        .filter_map(|id| {
                            run.runtime_actor(&id).and_then(|actor| {
                                crate::game::actor_satisfies_ability_access(run, ability, &id)
                                    .then_some((id, actor.name))
                            })
                        }),
                );
                recipients
            })
            .unwrap_or_default();
        if !recipients
            .iter()
            .any(|(id, _)| id == &proposal.recipient_id)
        {
            proposal.recipient_id = recipients
                .first()
                .map(|(id, _)| id.clone())
                .unwrap_or_default();
        }
        let mut accept = false;
        let mut decline = false;
        egui::Window::new("Make this ability grant real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Immutable ability template");
                egui::ComboBox::from_id_salt("narrator_ability_template")
                    .selected_text(&proposal.template_id)
                    .show_ui(ui, |ui| {
                        for ability in &bundle.abilities {
                            ui.selectable_value(
                                &mut proposal.template_id,
                                ability.id.clone(),
                                format!("{} ({})", ability.name, ability.id),
                            );
                        }
                    });
                ui.label("Recipient");
                egui::ComboBox::from_id_salt("narrator_ability_recipient")
                    .selected_text(
                        recipients
                            .iter()
                            .find(|(id, _)| id == &proposal.recipient_id)
                            .map(|(_, label)| label.as_str())
                            .unwrap_or("Player"),
                    )
                    .show_ui(ui, |ui| {
                        for (id, label) in &recipients {
                            ui.selectable_value(&mut proposal.recipient_id, id.clone(), label);
                        }
                    });
                if recipients.is_empty() {
                    ui.small("No present actor satisfies this ability's required access tags.");
                }
                ui.horizontal(|ui| {
                    ui.label("Uses (0 = infinite)");
                    let mut uses = proposal.remaining_uses.unwrap_or(0);
                    if ui.add(egui::DragValue::new(&mut uses).range(0..=999)).changed() {
                        proposal.remaining_uses = (uses != 0).then_some(uses);
                    }
                    ui.label("Duration turns (0 = permanent)");
                    let mut duration = proposal.duration_turns.unwrap_or(0);
                    if ui.add(egui::DragValue::new(&mut duration).range(0..=999)).changed() {
                        proposal.duration_turns = (duration != 0).then_some(duration);
                    }
                });
                ui.small("The narrator may only propose an existing template. The host selects a legal owner; reducer validation creates one independent grant with its own uses and expiry.");
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!recipients.is_empty(), egui::Button::new("Grant this ability"))
                        .clicked()
                    {
                        accept = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if accept {
            let owner = self
                .current_run
                .as_ref()
                .filter(|run| proposal.recipient_id == run.player_entity_id)
                .map(|run| run.controlled_inventory_owner())
                .unwrap_or_else(|| ItemOwner::Entity(proposal.recipient_id.clone()));
            let source = if proposal.duration_turns.is_some() {
                AbilityGrantSource::Temporary
            } else {
                AbilityGrantSource::Granted
            };
            if self.admit_narrator_ability_grant(
                owner,
                proposal.template_id.clone(),
                proposal.remaining_uses,
                proposal.duration_turns,
                source,
            ) {
                self.pending_narrator_ability_grant = None;
            } else {
                self.pending_narrator_ability_grant = Some(proposal);
            }
        } else if decline {
            self.pending_narrator_ability_grant = None;
            self.log_lines
                .push("Narrator ability grant declined: it remains flavor only.".to_owned());
        } else {
            self.pending_narrator_ability_grant = Some(proposal);
        }
        if self.pending_narrator_ability_grant.is_none()
            && !self.queued_narrator_ability_grants.is_empty()
        {
            self.pending_narrator_ability_grant =
                Some(self.queued_narrator_ability_grants.remove(0));
        }
    }

    fn show_narrator_temporary_exit_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_temporary_exit.clone() else {
            return;
        };
        let mut accept = false;
        let mut accept_and_save = false;
        let mut decline = false;
        egui::Window::new("Make this route real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.label("Description");
                ui.text_edit_multiline(&mut proposal.description);
                ui.small(format!(
                    "{} → {}{}",
                    proposal.from_location_id,
                    proposal.to_location_id,
                    if proposal.locked { " · locked" } else { "" }
                ));
                ui.horizontal(|ui| {
                    if ui.button("Make route real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if !accept && !accept_and_save && !decline {
            self.pending_narrator_temporary_exit = Some(proposal.clone());
        }
        if accept || accept_and_save {
            self.pending_narrator_temporary_exit = None;
            self.admit_narrator_temporary_exit(proposal.clone());
            if accept_and_save {
                self.save_narrator_temporary_exit_to_collection(&proposal);
            }
        } else if decline {
            self.pending_narrator_temporary_exit = None;
        }
        if self.pending_narrator_temporary_exit.is_none()
            && !self.queued_narrator_temporary_exits.is_empty()
        {
            self.pending_narrator_temporary_exit =
                Some(self.queued_narrator_temporary_exits.remove(0));
        }
    }

    fn show_narrator_objective_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_objective.clone() else {
            return;
        };
        let mut accept = false;
        let mut accept_and_save = false;
        let mut decline = false;
        egui::Window::new("Make this objective real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.label("Description");
                ui.text_edit_multiline(&mut proposal.description);
                ui.small(format!("Completes at: {}", proposal.completion_key));
                ui.horizontal(|ui| {
                    if ui.button("Make objective real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if !accept && !accept_and_save && !decline {
            self.pending_narrator_objective = Some(proposal.clone());
        }
        if accept || accept_and_save {
            self.pending_narrator_objective = None;
            self.admit_narrator_objective(proposal.clone());
            if accept_and_save {
                self.save_narrator_objective_to_collection(&proposal);
            }
        } else if decline {
            self.pending_narrator_objective = None;
        }
        if self.pending_narrator_objective.is_none() && !self.queued_narrator_objectives.is_empty()
        {
            self.pending_narrator_objective = Some(self.queued_narrator_objectives.remove(0));
        }
    }

    fn show_narrator_boss_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_boss.clone() else {
            return;
        };
        let mut accept = false;
        let mut accept_and_save = false;
        let mut decline = false;
        egui::Window::new("Make this boss real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.label("Description");
                ui.text_edit_multiline(&mut proposal.description);
                ui.horizontal(|ui| {
                    ui.label("HP");
                    ui.add(egui::DragValue::new(&mut proposal.max_hp).range(1..=999));
                    ui.label("Damage");
                    ui.add(egui::DragValue::new(&mut proposal.damage).range(0..=999));
                });
                show_editable_proposal_tags(ui, &mut proposal.tags);
                show_locked_encounter_contract(ui, proposal.max_hp, proposal.damage);
                ui.horizontal(|ui| {
                    if ui.button("Make boss real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if !accept && !accept_and_save && !decline {
            self.pending_narrator_boss = Some(proposal.clone());
        }
        if accept || accept_and_save {
            self.pending_narrator_boss = None;
            self.admit_narrator_boss(proposal.clone());
            if accept_and_save {
                self.save_narrator_boss_to_collection(&proposal);
            }
        } else if decline {
            self.pending_narrator_boss = None;
        }
        if self.pending_narrator_boss.is_none() && !self.queued_narrator_bosses.is_empty() {
            self.pending_narrator_boss = Some(self.queued_narrator_bosses.remove(0));
        }
    }

    fn show_narrator_enemy_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_enemy.clone() else {
            return;
        };
        let mut accept = false;
        let mut accept_and_save = false;
        let mut decline = false;
        egui::Window::new("Make this enemy real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.label("Description");
                ui.text_edit_multiline(&mut proposal.description);
                ui.horizontal(|ui| {
                    ui.label("HP");
                    ui.add(egui::DragValue::new(&mut proposal.max_hp).range(1..=999));
                    ui.label("Damage");
                    ui.add(egui::DragValue::new(&mut proposal.damage).range(0..=999));
                });
                show_editable_proposal_tags(ui, &mut proposal.tags);
                show_locked_encounter_contract(ui, proposal.max_hp, proposal.damage);
                ui.horizontal(|ui| {
                    if ui.button("Make enemy real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if !accept && !accept_and_save && !decline {
            self.pending_narrator_enemy = Some(proposal.clone());
        }
        if accept || accept_and_save {
            self.pending_narrator_enemy = None;
            self.admit_narrator_enemy(proposal.clone());
            if accept_and_save {
                self.save_narrator_enemy_to_collection(&proposal);
            }
        } else if decline {
            self.pending_narrator_enemy = None;
        }
        if self.pending_narrator_enemy.is_none() && !self.queued_narrator_enemies.is_empty() {
            self.pending_narrator_enemy = Some(self.queued_narrator_enemies.remove(0));
        }
    }

    fn show_narrator_npc_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_npc.clone() else {
            return;
        };
        let mut accept = false;
        let mut accept_and_save = false;
        let mut decline = false;
        egui::Window::new("Make this NPC real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.label("Description");
                ui.text_edit_multiline(&mut proposal.description);
                ui.small(format!("Faction: {}", proposal.faction));
                show_editable_proposal_tags(ui, &mut proposal.tags);
                show_actor_capability_contract(ui, &mut proposal.actor);
                ui.small(
                    "Source: narrator proposal. The reducer places this NPC only if you accept.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Make NPC real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if !accept && !accept_and_save && !decline {
            self.pending_narrator_npc = Some(proposal.clone());
        }
        if accept || accept_and_save {
            if self.admit_narrator_npc(proposal.clone()) {
                self.pending_narrator_npc = None;
                if accept_and_save {
                    self.save_narrator_npc_to_collection(&proposal);
                }
            } else {
                self.pending_narrator_npc = Some(proposal.clone());
            }
        } else if decline {
            self.pending_narrator_npc = None;
            self.log_lines
                .push("Narrator NPC proposal declined: it remains flavor only.".to_owned());
        }
        if self.pending_narrator_npc.is_none() && !self.queued_narrator_npcs.is_empty() {
            self.pending_narrator_npc = Some(self.queued_narrator_npcs.remove(0));
        }
    }

    fn show_narrator_status_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_status.clone() else {
            return;
        };
        let mut target_tags = proposal.required_target_tags.join(", ");
        let mut context_tags = proposal.required_context_tags.join(", ");
        let mut accept = false;
        let mut accept_and_save = false;
        let mut decline = false;
        egui::Window::new("Make this effect real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.horizontal(|ui| {
                    ui.label("Trigger");
                    egui::ComboBox::from_id_salt("status_trigger")
                        .selected_text(&proposal.trigger)
                        .show_ui(ui, |ui| {
                            for trigger in ["combat", "movement", "inspect", "wait", "failure"] {
                                ui.selectable_value(&mut proposal.trigger, trigger.to_owned(), trigger);
                            }
                        });
                    ui.label("Duration");
                    egui::ComboBox::from_id_salt("status_duration")
                        .selected_text(&proposal.duration)
                        .show_ui(ui, |ui| {
                            for duration in ["scene", "until_used", "run"] {
                                ui.selectable_value(&mut proposal.duration, duration.to_owned(), duration);
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Damage bonus");
                    ui.add(egui::DragValue::new(&mut proposal.additive_damage).range(-999..=999));
                    ui.label("Healing bonus");
                    ui.add(egui::DragValue::new(&mut proposal.additive_healing).range(-999..=999));
                });
                ui.label("Required target tags (comma-separated; none for any)");
                ui.text_edit_singleline(&mut target_tags);
                ui.label("Required context tags (comma-separated; none for any)");
                ui.text_edit_singleline(&mut context_tags);
                proposal.required_target_tags = normalize_tags(
                    target_tags.split(',').map(str::to_owned).collect(),
                );
                proposal.required_context_tags = normalize_tags(
                    context_tags.split(',').map(str::to_owned).collect(),
                );
                ui.small(
                    "Combat effects use damage; movement, inspect, wait, and failure effects use healing. The host rejects no-op combinations.",
                );
                ui.small("Source: narrator proposal. The reducer activates it only if you accept.");
                ui.horizontal(|ui| {
                    if ui.button("Make effect real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });
        if !accept && !accept_and_save && !decline {
            self.pending_narrator_status = Some(proposal.clone());
        }
        if accept || accept_and_save {
            self.pending_narrator_status = None;
            self.admit_narrator_status(proposal.clone());
            if accept_and_save {
                self.save_narrator_status_to_collection(&proposal);
            }
        } else if decline {
            self.pending_narrator_status = None;
            self.log_lines
                .push("Narrator status proposal declined: it remains flavor only.".to_owned());
        }
        if self.pending_narrator_status.is_none() && !self.queued_narrator_statuses.is_empty() {
            self.pending_narrator_status = Some(self.queued_narrator_statuses.remove(0));
        }
    }

    fn show_narrator_item_proposal(&mut self, ctx: &egui::Context) {
        let Some(mut proposal) = self.pending_narrator_item.clone() else {
            return;
        };

        let mut accept = false;
        let mut decline = false;
        let mut accept_and_save = false;
        egui::Window::new("Make this real?")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Name");
                ui.text_edit_singleline(&mut proposal.name);
                ui.label("Description");
                ui.text_edit_multiline(&mut proposal.description);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Attack damage");
                    ui.add(egui::DragValue::new(&mut proposal.damage).range(0..=999));
                });
                show_editable_proposal_tags(ui, &mut proposal.tags);
                ui.small(format!(
                    "Can be picked up: Yes · Can be equipped: Yes · Player attack contribution: {}",
                    proposal.damage
                ));
                show_actor_capability_contract(ui, &mut proposal.actor);
                if let Some(condition) = &proposal.condition {
                    ui.small(format!("Only when: {}", condition));
                }
                ui.small(
                    "Source: narrator proposal. It becomes saved session truth only if you accept.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Make it real this run").clicked() {
                        accept = true;
                    }
                    if ui.button("Make real & save for future games").clicked() {
                        accept_and_save = true;
                    }
                    if ui.button("No thanks").clicked() {
                        decline = true;
                    }
                });
            });

        if !accept && !accept_and_save && !decline {
            self.pending_narrator_item = Some(proposal.clone());
        }
        if accept || accept_and_save {
            if self.admit_narrator_item(proposal.clone()) {
                self.pending_narrator_item = None;
                if accept_and_save {
                    self.save_narrator_item_to_collection(&proposal);
                }
            } else {
                self.pending_narrator_item = Some(proposal.clone());
            }
        } else if decline {
            self.pending_narrator_item = None;
            self.log_lines.push(format!(
                "Narrator proposal declined: {} stays flavor only.",
                proposal.name
            ));
        }

        if self.pending_narrator_item.is_none() && !self.queued_narrator_items.is_empty() {
            self.pending_narrator_item = Some(self.queued_narrator_items.remove(0));
        }
    }

    fn admit_narrator_item(&mut self, proposal: NarratorItemProposal) -> bool {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return false;
        };
        let Some(run) = self.current_run.as_mut() else {
            return false;
        };
        let (actor, disposition) = match proposal.actor.effective_actor(&proposal.name) {
            Ok(contract) => contract,
            Err(error) => {
                self.log_lines.push(format!(
                    "Narrator item proposal rejected by host capability validator: {error}"
                ));
                return false;
            }
        };
        let action = GameAction::AdmitNarratorItem {
            name: proposal.name,
            description: proposal.description,
            damage: proposal.damage,
            condition: proposal.condition,
            tags: proposal.tags,
            actor,
            disposition,
        };
        let outcome = apply_action(run, bundle, action);
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
        true
    }

    fn admit_narrator_status(&mut self, proposal: NarratorStatusProposal) {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        let Some(run) = self.current_run.as_mut() else {
            return;
        };
        let outcome = apply_action(
            run,
            bundle,
            GameAction::AdmitNarratorStatus {
                name: proposal.name,
                trigger: proposal.trigger,
                duration: proposal.duration,
                additive_damage: proposal.additive_damage,
                additive_healing: proposal.additive_healing,
                required_target_tags: proposal.required_target_tags,
                required_context_tags: proposal.required_context_tags,
            },
        );
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
    }

    fn admit_narrator_npc(&mut self, proposal: NarratorNpcProposal) -> bool {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return false;
        };
        let Some(run) = self.current_run.as_mut() else {
            return false;
        };
        let (actor, disposition) = match proposal.actor.effective_actor(&proposal.name) {
            Ok((actor, disposition)) => (actor.unwrap_or_default(), disposition),
            Err(error) => {
                self.log_lines.push(format!(
                    "Narrator NPC proposal rejected by host capability validator: {error}"
                ));
                return false;
            }
        };
        let outcome = apply_action(
            run,
            bundle,
            GameAction::AdmitNarratorNpc {
                name: proposal.name,
                description: proposal.description,
                faction: proposal.faction,
                tags: proposal.tags,
                actor,
                disposition,
            },
        );
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
        true
    }

    fn admit_narrator_enemy(&mut self, proposal: NarratorEnemyProposal) {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        let Some(run) = self.current_run.as_mut() else {
            return;
        };
        let outcome = apply_action(
            run,
            bundle,
            GameAction::AdmitNarratorEnemy {
                name: proposal.name,
                description: proposal.description,
                max_hp: proposal.max_hp,
                damage: proposal.damage,
                tags: proposal.tags,
            },
        );
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
    }

    fn admit_narrator_temporary_exit(&mut self, proposal: NarratorTemporaryExitProposal) {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        let Some(run) = self.current_run.as_mut() else {
            return;
        };
        let outcome = apply_action(
            run,
            bundle,
            GameAction::AdmitNarratorTemporaryExit {
                name: proposal.name,
                description: proposal.description,
                from_location_id: proposal.from_location_id,
                to_location_id: proposal.to_location_id,
                locked: proposal.locked,
            },
        );
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
    }

    fn admit_narrator_ability_grant(
        &mut self,
        owner: ItemOwner,
        template_id: String,
        remaining_uses: Option<u32>,
        duration_turns: Option<u64>,
        source: AbilityGrantSource,
    ) -> bool {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return false;
        };
        let Some(run) = self.current_run.as_mut() else {
            return false;
        };
        let owner_id = match &owner {
            ItemOwner::Player => run.player_entity_id.as_str(),
            ItemOwner::Entity(entity_id) => entity_id.as_str(),
            ItemOwner::Location(_) => {
                self.log_lines.push(
                    "Narrator ability grant rejected: a location is not an actor recipient."
                        .to_owned(),
                );
                return false;
            }
        };
        let Some(template) = bundle
            .abilities
            .iter()
            .find(|template| template.id == template_id)
        else {
            self.log_lines.push(format!(
                "Narrator ability grant rejected: unknown template '{template_id}'."
            ));
            return false;
        };
        if !crate::game::actor_satisfies_ability_access(run, template, owner_id) {
            self.log_lines.push(format!(
                "Narrator ability grant rejected: '{owner_id}' lacks the access tags for '{}'.",
                template.name
            ));
            return false;
        }
        let action = GameAction::GrantAbility {
            owner,
            template_id,
            remaining_uses,
            duration_turns,
            source,
        };
        let outcome = apply_action(run, bundle, action.clone());
        let accepted = outcome
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::AbilityGranted { .. }));
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(
            action_traces(&action, &outcome)
                .into_iter()
                .map(|trace| trace.chat_line()),
        );
        self.log_lines.extend(outcome.lines);
        accepted
    }

    fn save_narrator_temporary_exit_to_collection(
        &mut self,
        proposal: &NarratorTemporaryExitProposal,
    ) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let template = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::TemporaryExit,
            name: proposal.name.clone(),
            description: proposal.description.clone(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::TemporaryExit {
                from_location_id: proposal.from_location_id.clone(),
                to_location_id: proposal.to_location_id.clone(),
                locked: proposal.locked,
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(template) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn admit_narrator_objective(&mut self, proposal: NarratorObjectiveProposal) {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        let Some(run) = self.current_run.as_mut() else {
            return;
        };
        let outcome = apply_action(
            run,
            bundle,
            GameAction::AdmitNarratorObjective {
                name: proposal.name,
                description: proposal.description,
                completion_key: proposal.completion_key,
            },
        );
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
    }

    fn save_narrator_objective_to_collection(&mut self, proposal: &NarratorObjectiveProposal) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let template = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::Objective,
            name: proposal.name.clone(),
            description: proposal.description.clone(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Objective {
                completion_key: proposal.completion_key.clone(),
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(template) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn admit_narrator_boss(&mut self, proposal: NarratorBossProposal) {
        let Some(bundle) = self.current_bundle.as_ref() else {
            return;
        };
        let Some(run) = self.current_run.as_mut() else {
            return;
        };
        let outcome = apply_action(
            run,
            bundle,
            GameAction::AdmitNarratorBoss {
                name: proposal.name,
                description: proposal.description,
                max_hp: proposal.max_hp,
                damage: proposal.damage,
                tags: proposal.tags,
            },
        );
        self.diagnostic_events
            .extend(outcome.events.iter().cloned());
        self.log_lines.extend(outcome.lines);
    }

    fn save_narrator_boss_to_collection(&mut self, proposal: &NarratorBossProposal) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let template = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::Boss,
            name: proposal.name.clone(),
            description: proposal.description.clone(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Boss {
                max_hp: proposal.max_hp,
                damage: proposal.damage,
            }),
            tags: proposal.tags.clone(),
            actor: None,
            disposition: ActorDisposition::Hostile,
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(template) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn save_narrator_enemy_to_collection(&mut self, proposal: &NarratorEnemyProposal) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let template = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::Enemy,
            name: proposal.name.clone(),
            description: proposal.description.clone(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Enemy {
                max_hp: proposal.max_hp,
                damage: proposal.damage,
            }),
            tags: proposal.tags.clone(),
            actor: None,
            disposition: ActorDisposition::Hostile,
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(template) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn save_narrator_npc_to_collection(&mut self, proposal: &NarratorNpcProposal) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let template = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::Npc,
            name: proposal.name.clone(),
            description: proposal.description.clone(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Npc {
                faction: proposal.faction.clone(),
            }),
            tags: proposal.tags.clone(),
            actor: proposal
                .actor
                .effective_actor(&proposal.name)
                .ok()
                .and_then(|(actor, _)| actor),
            disposition: proposal.actor.disposition.clone(),
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(template) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn save_narrator_item_to_collection(&mut self, proposal: &NarratorItemProposal) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let item = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::Weapon,
            name: proposal.name.clone(),
            description: proposal.description.clone(),
            damage: proposal.damage,
            condition: proposal.condition.clone(),
            mechanics: Some(PlayerTemplateMechanics::Item {
                damage: proposal.damage,
                condition: proposal.condition.clone(),
            }),
            tags: proposal.tags.clone(),
            actor: proposal
                .actor
                .effective_actor(&proposal.name)
                .ok()
                .and_then(|(actor, _)| actor),
            disposition: proposal.actor.disposition.clone(),
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(item) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn save_narrator_status_to_collection(&mut self, proposal: &NarratorStatusProposal) {
        let Some(run) = self.current_run.as_ref() else {
            return;
        };
        let template = CollectionItem {
            id: collection_item_id(&run.datapack_id, &proposal.name),
            kind: PlayerTemplateKind::Status,
            name: proposal.name.clone(),
            description: format!(
                "Narrator-approved {} effect lasting {}.",
                proposal.trigger, proposal.duration
            ),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Status {
                trigger: proposal.trigger.clone(),
                duration: proposal.duration.clone(),
                additive_damage: proposal.additive_damage,
                additive_healing: proposal.additive_healing,
                required_target_tags: proposal.required_target_tags.clone(),
                required_context_tags: proposal.required_context_tags.clone(),
            }),
            tags: proposal.required_target_tags.clone(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: run.datapack_id.clone(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };
        match save_player_template(template) {
            Ok(path) => self.log_lines.push(format!(
                "Personal collection saved: {} effect is available for future {} runs ({path}).",
                proposal.name, run.datapack_display_name
            )),
            Err(error) => self.log_lines.push(format!("System: {error}")),
        }
    }

    fn save_current_run(&mut self) {
        let Some(run_state) = self.current_run.clone() else {
            self.log_lines
                .push("No active run exists to save.".to_owned());
            return;
        };

        let payload = build_save_payload(
            current_save_version(),
            &self.setup.to_snapshot(),
            game_tab_key(self.ui_session.active_tab),
            &self.log_lines,
            &self.diagnostic_events,
            &run_state,
        );

        match save_game(&payload) {
            Ok(path) => self
                .log_lines
                .push(format!("System: Save written to {}.", path)),
            Err(error) => self.log_lines.push(format!("System: {}", error)),
        }
    }

    fn load_saved_run(&mut self) {
        match load_game() {
            Ok(payload) => {
                let restored = unpack_save_payload(payload);
                self.setup.apply_snapshot(restored.setup.clone());
                self.persist_title_setup();
                self.ui_session.active_tab = parse_game_tab_key(&restored.active_tab_key);
                self.ui_session.current_input.clear();
                self.ui_session.asset_viewer_request = None;
                self.diagnostic_events = restored.diagnostic_events;
                self.loaded_save_version = Some(restored.save_version);
                self.log_lines = restored.log_lines;
                self.current_run = Some(restored.run_state.clone());

                match restore_bundle_for_run(&self.datapacks, &restored.run_state) {
                    Ok(bundle) => {
                        let pack_uses_narration = bundle.rules.narration.usage != "unused";
                        let narrator = (pack_uses_narration
                            && self.setup.narrator_backend != "None")
                            .then(|| build_narrator(&bundle, &self.setup));
                        let mock_narrator = pack_uses_narration
                            .then(|| MockNarrator::new(&bundle, self.setup.chaos_mode));
                        self.current_bundle = Some(bundle);
                        self.mock_narrator = mock_narrator;
                        self.narrator = narrator;
                        self.screen = AppScreen::ActiveRun;
                        self.log_lines
                            .push("System: Save loaded successfully.".to_owned());
                    }
                    Err(RestoreBundleError::InvalidBundle(errors)) => {
                        self.current_bundle = None;
                        self.narrator = None;
                        self.screen = AppScreen::Setup;
                        self.log_lines.push(
                            "System: Save file loaded, but datapack bundle could not be restored."
                                .to_owned(),
                        );
                        for error in errors {
                            self.log_lines.push(format!("Validation error: {}", error));
                        }
                    }
                    Err(RestoreBundleError::MissingDatapack) => {
                        self.current_bundle = None;
                        self.narrator = None;
                        self.screen = AppScreen::Setup;
                        self.log_lines.push(
                            "System: Save file loaded, but the referenced datapack is not currently available."
                                .to_owned(),
                        );
                    }
                }
            }
            Err(error) => {
                self.log_lines.push(format!("System: {}", error));
            }
        }
    }
}

fn effect_trigger_from_template(value: &str) -> EffectTrigger {
    match value.trim().to_ascii_lowercase().as_str() {
        "movement" => EffectTrigger::Movement,
        "inspect" => EffectTrigger::Inspect,
        "wait" => EffectTrigger::Wait,
        "failure" => EffectTrigger::Failure,
        _ => EffectTrigger::Combat,
    }
}

fn effect_duration_from_template(value: &str) -> EffectDuration {
    match value.trim().to_ascii_lowercase().as_str() {
        "scene" => EffectDuration::Scene,
        "until_used" | "until-used" => EffectDuration::UntilUsed,
        _ => EffectDuration::Run,
    }
}

impl eframe::App for ChattyQuestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_narration_task(ctx);
        match self.screen {
            AppScreen::Splash => self.show_splash_screen(ctx),
            AppScreen::Setup => self.show_setup_screen(ctx),
            AppScreen::ActiveRun => self.show_game_shell(ctx),
        }
    }
}

fn game_tab_key(tab: GameTab) -> &'static str {
    match tab {
        GameTab::Game => "game",
        GameTab::Inventory => "inventory",
        GameTab::Character => "character",
        GameTab::Diagnostics => "diagnostics",
        GameTab::Library => "library",
    }
}

fn parse_game_tab_key(value: &str) -> GameTab {
    match value {
        "inventory" => GameTab::Inventory,
        "character" => GameTab::Character,
        "diagnostics" => GameTab::Diagnostics,
        "library" => GameTab::Library,
        _ => GameTab::Game,
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(18, 22, 18);
    visuals.window_fill = egui::Color32::from_rgb(24, 29, 24);
    visuals.extreme_bg_color = egui::Color32::from_rgb(9, 12, 10);
    visuals.hyperlink_color = egui::Color32::from_rgb(152, 214, 136);
    visuals.selection.bg_fill = egui::Color32::from_rgb(78, 104, 62);
    ctx.set_visuals(visuals);
}

fn asset_if_present(path: &'static str) -> Option<String> {
    let resolved = app_paths::path(path);
    resolved.exists().then(|| resolved.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::{ProposalActorConfiguration, enabled_player_templates};
    use crate::collection::{
        PlayerTemplate, PlayerTemplateAdmission, PlayerTemplateKind, PlayerTemplateMechanics,
        PlayerTemplateMediaReferences,
    };
    use crate::game::ActorDisposition;

    fn template(id: &str, kind: PlayerTemplateKind, enabled: bool) -> PlayerTemplate {
        PlayerTemplate {
            id: id.to_owned(),
            kind,
            name: id.to_owned(),
            description: "test template".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Item {
                damage: 0,
                condition: None,
            }),
            tags: Vec::new(),
            actor: None,
            disposition: ActorDisposition::Neutral,
            source_datapack_id: "test_pack".to_owned(),
            enabled,
            provenance: "test".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        }
    }

    #[test]
    fn disabled_player_templates_are_removed_before_all_family_materialization() {
        let templates = enabled_player_templates(vec![
            template("enabled_item", PlayerTemplateKind::Item, true),
            template("disabled_item", PlayerTemplateKind::Item, false),
            template("disabled_npc", PlayerTemplateKind::Npc, false),
            template("disabled_effect", PlayerTemplateKind::Effect, false),
        ]);

        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].id, "enabled_item");
    }

    #[test]
    fn narrator_inventory_checkbox_reaches_the_effective_actor_contract() {
        let draft = ProposalActorConfiguration::from_narrator(
            crate::game::narrator::NarratorActorConfiguration {
                can_carry_inventory: true,
                ..Default::default()
            },
        );
        let (actor, _) = draft
            .effective_actor("fixture")
            .expect("inventory-only actor contract is valid");
        assert!(actor.is_some_and(|actor| {
            actor.has_capability(crate::game::capabilities::EngineCapability::Inventory)
        }));
    }
}
