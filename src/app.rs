use std::path::Path;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::data::datapacks::{
    DatapackBundle, DatapackCatalog, DatapackRecord, datapack_schema_version, discover_datapacks,
    load_datapack_bundle_by_folder,
};
use crate::diagnostics::build_diagnostic_report;
use crate::game::narrator::{MockNarrator, Narrator};
use crate::game::{
    ActionOutcome, GameEvent, GeneratedRun, RunState, apply_action, generate_new_run, parse_action,
};
use crate::media::build_media_panel_state;
use crate::runtime::{current_save_path, current_save_version, load_game, save_game};
use crate::session::{
    RestoreBundleError, SessionSetupSnapshot, build_save_payload, restore_bundle_for_run,
    unpack_save_payload,
};
use crate::ui::{
    AssetViewerRequest, SelectedDatapackPreview, SetupScreenAction, SetupScreenModel,
    SplashScreenModel, show_asset_viewer, show_character_tab, show_diagnostics_tab,
    show_game_action_bar, show_game_tab, show_inventory_tab, show_setup_screen, show_splash_screen,
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
}

struct SetupConfig {
    selected_datapack: String,
    difficulty: f32,
    chaos_mode: f32,
    fog_mode: String,
    dm_capsule: String,
    cpu_helper_model: String,
    gpu_narrator_model: String,
}

impl SetupConfig {
    fn to_snapshot(&self) -> SessionSetupSnapshot {
        SessionSetupSnapshot {
            selected_datapack: self.selected_datapack.clone(),
            difficulty: self.difficulty,
            chaos_mode: self.chaos_mode,
            fog_mode: self.fog_mode.clone(),
            dm_capsule: self.dm_capsule.clone(),
            cpu_helper_model: self.cpu_helper_model.clone(),
            gpu_narrator_model: self.gpu_narrator_model.clone(),
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
    }
}

struct UiSessionState {
    active_tab: GameTab,
    current_input: String,
    asset_viewer_request: Option<AssetViewerRequest>,
}

struct SplashState {
    started_at: Instant,
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
    log_lines: Vec<String>,
    diagnostic_events: Vec<GameEvent>,
    loaded_save_version: Option<u32>,
}

impl ChattyQuestApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let datapacks = discover_datapacks();
        let selected_datapack = datapacks
            .valid
            .first()
            .map(|record| record.summary.display_name.clone())
            .unwrap_or_else(|| "No valid datapacks found".to_owned());

        Self {
            screen: AppScreen::Splash,
            setup: SetupConfig {
                selected_datapack,
                difficulty: 0.35,
                chaos_mode: 0.10,
                fog_mode: "Known".to_owned(),
                dm_capsule: "Hostile Meatgrinder".to_owned(),
                cpu_helper_model: "Reserved: Bookkeeper / Summary Helper".to_owned(),
                gpu_narrator_model: "Reserved: Main Narrator".to_owned(),
            },
            ui_session: UiSessionState {
                active_tab: GameTab::Game,
                current_input: String::new(),
                asset_viewer_request: None,
            },
            splash: SplashState {
                started_at: Instant::now(),
            },
            datapacks,
            current_bundle: None,
            current_run: None,
            narrator: None,
            log_lines: vec![
                format!(
                    "{} is ready for a local run on the {}.",
                    APP_DISPLAY_NAME, ENGINE_DISPLAY_NAME
                ),
                "Datapack discovery reads the local assets/datapacks folder.".to_owned(),
                "Generate a scenario or load the current save to enter the tabbed game shell."
                    .to_owned(),
            ],
            diagnostic_events: Vec::new(),
            loaded_save_version: None,
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
                    boundary_response: record.summary.boundary_response.clone(),
                    dm_style_preview: record.summary.dm_style_preview.clone(),
                    world_tone_preview: record.summary.world_tone_preview.clone(),
                });
        let action = show_setup_screen(
            ctx,
            SetupScreenModel {
                selected_datapack: &mut self.setup.selected_datapack,
                difficulty: &mut self.setup.difficulty,
                chaos_mode: &mut self.setup.chaos_mode,
                fog_mode: &mut self.setup.fog_mode,
                dm_capsule: &mut self.setup.dm_capsule,
                cpu_helper_model: &mut self.setup.cpu_helper_model,
                gpu_narrator_model: &mut self.setup.gpu_narrator_model,
                datapacks: &self.datapacks,
                selected_record,
                header_image_path: asset_if_present(SETUP_HEADER_IMAGE_PATH),
            },
        );

        match action {
            SetupScreenAction::None => {}
            SetupScreenAction::GenerateGame => self.start_new_run(),
            SetupScreenAction::LoadGame => self.load_saved_run(),
        }
    }

    fn show_game_shell(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Setup").clicked() {
                    self.screen = AppScreen::Setup;
                }

                if ui.button("Save").clicked() {
                    self.save_current_run();
                }

                if ui.button("Load").clicked() {
                    self.load_saved_run();
                }

                ui.separator();
                ui.small(format!("{} | Radiant Determinism", ENGINE_DISPLAY_NAME));
                ui.separator();
                ui.label(format!("Run: {}", self.setup.selected_datapack));
                ui.separator();
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
            });
        });

        let mut queued_command = None;

        if self.ui_session.active_tab == GameTab::Game {
            queued_command = show_game_action_bar(
                ctx,
                self.current_run.as_ref(),
                self.current_bundle.as_ref(),
                &mut self.ui_session.current_input,
            );
        }

        match self.ui_session.active_tab {
            GameTab::Game => {
                let media_panel = build_media_panel_state(
                    self.current_bundle.as_ref(),
                    self.current_run.as_ref(),
                    &self.diagnostic_events,
                );
                if queued_command.is_none() {
                    let (command, viewer_request) = show_game_tab(
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
                } else {
                    let (_, viewer_request) = show_game_tab(
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
                let viewer_request = show_character_tab(
                    ctx,
                    self.current_run.as_ref(),
                    self.current_bundle.as_ref(),
                );
                if viewer_request.is_some() {
                    self.ui_session.asset_viewer_request = viewer_request;
                }
            }
            GameTab::Diagnostics => {
                let report = build_diagnostic_report(
                    &self.datapacks,
                    self.current_bundle.as_ref(),
                    self.current_run.as_ref(),
                    self.narrator.is_some(),
                    current_save_path(),
                    current_save_version(),
                    self.loaded_save_version,
                    datapack_schema_version(),
                    &self.diagnostic_events,
                );
                show_diagnostics_tab(ctx, &report)
            }
        }

        if let Some(request) = self.ui_session.asset_viewer_request.as_ref()
            && !show_asset_viewer(ctx, request)
        {
            self.ui_session.asset_viewer_request = None;
        }

        if let Some(command) = queued_command {
            self.submit_command_text(command);
        }
    }

    fn selected_datapack_record(&self) -> Option<&DatapackRecord> {
        self.datapacks
            .valid
            .iter()
            .find(|record| record.summary.display_name == self.setup.selected_datapack)
    }

    fn start_new_run(&mut self) {
        let Some(record) = self.selected_datapack_record() else {
            self.log_lines
                .push("Cannot generate a run without a valid datapack.".to_owned());
            return;
        };

        match load_datapack_bundle_by_folder(&record.folder_name) {
            Ok(bundle) => {
                let GeneratedRun { state, log_lines } = generate_new_run(&bundle);
                let narrator = MockNarrator::new(&bundle, self.setup.chaos_mode);
                let narrated_start = narrator.narrate_run_start(&bundle, &state, &log_lines);
                self.current_bundle = Some(bundle);
                self.current_run = Some(state.clone());
                self.narrator = Some(Box::new(narrator));
                self.screen = AppScreen::ActiveRun;
                self.ui_session.active_tab = GameTab::Game;
                self.ui_session.current_input.clear();
                self.ui_session.asset_viewer_request = None;
                self.log_lines.clear();
                self.diagnostic_events.clear();
                self.loaded_save_version = Some(current_save_version());
                self.log_lines.extend(narrated_start);
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

        let action = match parse_action(&input) {
            Ok(action) => action,
            Err(error) => {
                self.log_lines.push(error);
                return;
            }
        };

        let raw_outcome = apply_action(run, bundle, action.clone());
        self.diagnostic_events
            .extend(raw_outcome.events.iter().cloned());
        let narrated_lines = if let Some(narrator) = self.narrator.as_ref() {
            narrator.narrate_action(bundle, &action, &raw_outcome, run)
        } else {
            let ActionOutcome { lines, .. } = raw_outcome;
            lines
        };
        self.log_lines.extend(narrated_lines);
    }

    fn submit_command_text(&mut self, command: String) {
        self.ui_session.current_input = command;
        self.submit_current_input();
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
                self.ui_session.active_tab = parse_game_tab_key(&restored.active_tab_key);
                self.ui_session.current_input.clear();
                self.ui_session.asset_viewer_request = None;
                self.diagnostic_events = restored.diagnostic_events;
                self.loaded_save_version = Some(restored.save_version);
                self.log_lines = restored.log_lines;
                self.current_run = Some(restored.run_state.clone());

                match restore_bundle_for_run(&self.datapacks, &restored.run_state) {
                    Ok(bundle) => {
                        let narrator = MockNarrator::new(&bundle, self.setup.chaos_mode);
                        self.current_bundle = Some(bundle);
                        self.narrator = Some(Box::new(narrator));
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

impl eframe::App for ChattyQuestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
    }
}

fn parse_game_tab_key(value: &str) -> GameTab {
    match value {
        "inventory" => GameTab::Inventory,
        "character" => GameTab::Character,
        "diagnostics" => GameTab::Diagnostics,
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

fn asset_if_present(path: &'static str) -> Option<&'static str> {
    Path::new(path).exists().then_some(path)
}
