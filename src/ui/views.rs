use std::collections::HashMap;

use eframe::egui;

use crate::collection::{PlayerTemplate, PlayerTemplateKind, PlayerTemplateMechanics};
use crate::data::datapacks::{DatapackBundle, DatapackCatalog};
use crate::diagnostics::DiagnosticReport;
use crate::game::RunState;
use crate::media::MediaPanelState;
use crate::ui::{
    MapTileHoverModel, MapTileVisibilityModel, build_asset_viewer_chrome, build_character_actions,
    build_character_summary, build_diagnostics_summary, build_game_action_bar_with_focus,
    build_game_header_chips, build_game_sidebar, build_inventory_action_rows,
    build_inventory_thumbnail_model, build_item_asset_viewer_request, build_local_item_action_rows,
    build_location_asset_viewer_request, build_map_exit_buttons, build_map_layout,
    build_map_legend, build_map_location_asset_viewer_request, build_map_tile_display,
    build_map_tile_hover, build_media_panel_asset_viewer_request, build_media_panel_display,
    build_media_panel_preview_model, build_outcome_banner,
};

#[derive(Clone)]
pub struct AssetViewerRequest {
    pub viewer_id: String,
    pub source_kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub description: Option<String>,
    pub image_path: Option<String>,
    pub resolved_source_label: Option<String>,
    pub using_datapack_fallback: bool,
    pub using_engine_fallback: bool,
}

#[derive(Default)]
pub struct LibraryUiState {
    feedback: Option<LibraryFeedback>,
    pending_removal: Option<LibraryRemovalTarget>,
    editing_tags: Option<LibraryTagEdit>,
    editing_template_media: Option<LibraryTemplateMediaEdit>,
    import_path: String,
    import_tags: String,
}

struct LibraryFeedback {
    message: String,
    is_error: bool,
}

enum LibraryRemovalTarget {
    Template { id: String, name: String },
    Media { path: String },
}

struct LibraryTagEdit {
    target: LibraryTagTarget,
    text: String,
}

struct LibraryTemplateMediaEdit {
    template_id: String,
    image: String,
    motion: String,
    audio: String,
}

enum LibraryTagTarget {
    Template(String),
    Media(String),
}

pub(crate) struct MapTileLayout {
    pub(crate) location_id: String,
    pub(crate) name: String,
    pub(crate) grid_x: usize,
    pub(crate) grid_y: usize,
    pub(crate) thumbnail_path: String,
    pub(crate) using_datapack_fallback: bool,
    pub(crate) using_engine_fallback: bool,
    pub(crate) has_items: bool,
    pub(crate) has_live_threats: bool,
    pub(crate) has_objective_target: bool,
    pub(crate) is_connected_to_current: bool,
    pub(crate) connects_north: bool,
    pub(crate) connects_east: bool,
    pub(crate) connects_south: bool,
    pub(crate) connects_west: bool,
}

pub(crate) struct GeneratedMapLayout {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) tiles: Vec<MapTileLayout>,
}

pub enum SetupScreenAction {
    None,
    GenerateGame,
    LoadGame,
    RefreshCartridges,
}

pub struct SetupScreenModel<'a> {
    pub selected_datapack: &'a mut String,
    pub selected_datapack_label: String,
    pub difficulty: &'a mut f32,
    pub chaos_mode: &'a mut f32,
    pub fog_mode: &'a mut String,
    pub dm_capsule: &'a mut String,
    pub cpu_helper_model: &'a mut String,
    pub gpu_narrator_model: &'a mut String,
    pub narrator_backend: &'a mut String,
    pub narrator_endpoint: &'a mut String,
    pub byo_provider: &'a mut String,
    pub byo_model: &'a mut String,
    pub byo_endpoint: &'a mut String,
    pub byo_api_key: &'a mut String,
    pub narration_preferences: &'a mut crate::game::narrator::NarrationPreferences,
    pub local_narrator_models: Vec<String>,
    pub allow_narrator_generated_additions: &'a mut bool,
    pub generated_proposal_kinds: &'a mut Vec<String>,
    pub play_preset: &'a mut String,
    pub play_contract_summary: Vec<String>,
    pub datapacks: &'a DatapackCatalog,
    pub selected_record: Option<SelectedDatapackPreview>,
    pub selected_invalid: Option<SelectedInvalidDatapackPreview>,
    pub header_image_path: Option<String>,
}

pub struct SplashScreenModel<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub supporting_line: Option<&'a str>,
    pub footer: &'a str,
    pub image_path: Option<String>,
    pub progress: f32,
}

#[derive(Clone)]
pub struct SelectedDatapackPreview {
    pub id: String,
    pub folder_name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub primary_scenario: String,
    pub location_count: usize,
    pub item_count: usize,
    pub enemy_count: usize,
    pub boss_count: usize,
    pub objective_count: usize,
    pub narrator_brief_count: usize,
    pub media_reference_count: usize,
    pub sensory_template_count: usize,
    pub boundary_response: Option<String>,
    pub dm_style_preview: Option<String>,
    pub world_tone_preview: Option<String>,
    pub required_engine_version: Option<String>,
    pub license: Option<String>,
    pub attribution: Option<String>,
    pub capabilities: Vec<String>,
}

#[derive(Clone)]
pub struct SelectedInvalidDatapackPreview {
    pub folder_name: String,
    pub display_name: Option<String>,
    pub version: Option<String>,
    pub errors: Vec<String>,
}

fn slider_node(value: f32) -> usize {
    (value.clamp(0.0, 1.0) * 3.0).round() as usize
}

fn apply_byo_provider_defaults(provider: &str, model: &mut String, endpoint: &mut String) {
    let (default_model, default_endpoint) = match provider {
        "OpenAI" => ("gpt-5-mini", "https://api.openai.com/v1/chat/completions"),
        "xAI" => ("grok-4.7", "https://api.x.ai/v1/chat/completions"),
        "Anthropic" => ("claude-sonnet-4-6", "https://api.anthropic.com/v1/messages"),
        "Google Gemini" => (
            "gemini-3.5-flash",
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
        ),
        _ => ("", "https://example.com/v1/chat/completions"),
    };
    *model = default_model.to_owned();
    *endpoint = default_endpoint.to_owned();
}

fn pressure_node_copy(value: f32) -> (&'static str, &'static str) {
    match slider_node(value) {
        0 => (
            "Gentle",
            "Lower mechanical pressure; the scenario gives more room to recover.",
        ),
        1 => (
            "Standard",
            "Balanced authored pressure for a normal scenario run.",
        ),
        2 => (
            "Pressed",
            "Higher mechanical pressure; mistakes and threats matter sooner.",
        ),
        _ => (
            "Brutal",
            "Maximum mechanical pressure within the authored rules.",
        ),
    }
}

fn narration_node_copy(value: f32) -> (&'static str, &'static str) {
    match slider_node(value) {
        0 => (
            "Tight",
            "Concise, factual presentation with minimal embellishment.",
        ),
        1 => (
            "Guided",
            "Controlled flavor around reducer-confirmed events.",
        ),
        2 => (
            "Loose",
            "More expressive narration, while the reducer still owns truth.",
        ),
        _ => (
            "Wild",
            "Maximum presentation freedom; no prose can change game state.",
        ),
    }
}

fn show_slider_nodes(ui: &mut egui::Ui, value: &mut f32, labels: [&str; 4]) {
    ui.horizontal_wrapped(|ui| {
        for (index, label) in labels.into_iter().enumerate() {
            let node_value = index as f32 / 3.0;
            if ui
                .selectable_label(slider_node(*value) == index, label)
                .clicked()
            {
                *value = node_value;
            }
        }
    });
}

pub fn show_setup_screen(ctx: &egui::Context, model: SetupScreenModel<'_>) -> SetupScreenAction {
    let mut action = SetupScreenAction::None;

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
        ui.add_space(24.0);
        if let Some(image_path) = model.header_image_path {
            let image_uri = local_file_uri(&image_path);
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Image::from_uri(image_uri)
                        .max_height(170.0)
                        .max_width(ui.available_width().min(760.0))
                        .corner_radius(egui::CornerRadius::same(14)),
                );
            });
            ui.add_space(10.0);
        }
        ui.heading("Chatty Quest");
        ui.small("Built on the RD Engine.");
        ui.label("A deterministic adventure game with a chat-forward Dungeon Master surface.");
        ui.small("Templates define what is real. Reducers decide what changes. Narration sells the hit.");
        ui.label("Setup comes first. Gameplay tabs appear once a run exists.");
        ui.add_space(18.0);

        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ui.group(|ui| {
                    ui.set_max_width(460.0);
                    ui.heading("Scenario Setup");
                    ui.label("Selected datapack");
                    egui::ComboBox::from_id_salt("datapack_select")
                        .selected_text(model.selected_datapack_label.as_str())
                        .show_ui(ui, |ui| {
                            ui.label("Ready cartridges");
                            for record in &model.datapacks.valid {
                                ui.selectable_value(
                                    model.selected_datapack,
                                    record.folder_name.clone(),
                                    format!("{} [ready]", record.summary.display_name),
                                );
                            }
                            if !model.datapacks.invalid.is_empty() {
                                ui.separator();
                                ui.label("Broken cartridges");
                                for record in &model.datapacks.invalid {
                                    let label = record
                                        .display_name
                                        .as_deref()
                                        .unwrap_or(record.folder_name.as_str());
                                    ui.selectable_value(
                                        model.selected_datapack,
                                        record.folder_name.clone(),
                                        format!("{label} [broken]"),
                                    );
                                }
                            }
                        });

                    if let Some(record) = &model.selected_record {
                        ui.add_space(8.0);
                        ui.small(format!(
                            "{} ({}) v{} by {}",
                            record.id,
                            record.folder_name,
                            record.version,
                            record.author
                        ));
                        ui.label(&record.description);
                        ui.small(format!(
                            "Scenario: {} | Locations: {} | Items: {} | Enemies: {} | Bosses: {} | Objectives: {}",
                            record.primary_scenario,
                            record.location_count,
                            record.item_count,
                            record.enemy_count,
                            record.boss_count,
                            record.objective_count
                        ));
                        ui.small(format!(
                            "Presentation coverage: {} narrator briefs | {} media refs | {} sensory templates",
                            record.narrator_brief_count,
                            record.media_reference_count,
                            record.sensory_template_count
                        ));
                        if let Some(required_engine_version) = &record.required_engine_version {
                            ui.small(format!(
                                "Required console/API version: {}",
                                required_engine_version
                            ));
                        }
                        if !record.capabilities.is_empty() {
                            ui.small(format!(
                                "Capabilities: {}",
                                record.capabilities.join(", ")
                            ));
                        }
                        if let Some(license) = &record.license {
                            ui.small(format!("License: {}", license));
                        }
                        if let Some(attribution) = &record.attribution {
                            ui.small(format!("Attribution: {}", attribution));
                        }
                    } else if let Some(invalid) = &model.selected_invalid {
                        ui.add_space(8.0);
                        let label = invalid
                            .display_name
                            .as_deref()
                            .unwrap_or(invalid.folder_name.as_str());
                        ui.colored_label(
                            egui::Color32::from_rgb(180, 40, 40),
                            format!("{} cannot boot.", label),
                        );
                        ui.small(format!("Folder: {}", invalid.folder_name));
                        if let Some(version) = &invalid.version {
                            ui.small(format!("Manifest version: {}", version));
                        }
                        for error in &invalid.errors {
                            ui.small(format!("- {}", error));
                        }
                    }

                    ui.add_space(8.0);
                    ui.label("Difficulty");
                    let pressure_slider = ui.add(
                        egui::Slider::new(model.difficulty, 0.0..=1.0)
                            .step_by(1.0 / 3.0)
                            .custom_formatter(|value, _| pressure_node_copy(value as f32).0.to_owned())
                            .text("Difficulty"),
                    );
                    if pressure_slider.changed() {
                        *model.difficulty = slider_node(*model.difficulty) as f32 / 3.0;
                    }
                    show_slider_nodes(
                        ui,
                        model.difficulty,
                        ["Gentle", "Standard", "Pressed", "Brutal"],
                    );
                    let (pressure_label, pressure_description) = pressure_node_copy(*model.difficulty);
                    ui.small(format!("{pressure_label}: {pressure_description}"));

                    ui.add_space(8.0);
                    ui.label("Chaos mode");
                    let narration_slider = ui.add(
                        egui::Slider::new(model.chaos_mode, 0.0..=1.0)
                            .step_by(1.0 / 3.0)
                            .custom_formatter(|value, _| narration_node_copy(value as f32).0.to_owned())
                            .text("Chaos"),
                    );
                    if narration_slider.changed() {
                        *model.chaos_mode = slider_node(*model.chaos_mode) as f32 / 3.0;
                    }
                    show_slider_nodes(
                        ui,
                        model.chaos_mode,
                        ["Tight", "Guided", "Loose", "Wild"],
                    );
                    let (narration_label, narration_description) = narration_node_copy(*model.chaos_mode);
                    ui.small(format!("{narration_label}: {narration_description}"));

                    ui.add_space(8.0);
                    ui.checkbox(
                        model.allow_narrator_generated_additions,
                        "Ask before narrator inventions become real",
                    );
                    ui.small("When enabled, proposed items need your approval before the reducer adds them to this run.");
                    ui.small("Chaos controls how often the host rolls. This whitelist controls which proposal families may be selected at all.");
                    ui.label("Random emergence allowed");
                    ui.horizontal_wrapped(|ui| {
                        for (kind, label) in [
                            ("item", "Items"),
                            ("npc", "NPCs"),
                            ("enemy", "Enemies"),
                            ("boss", "Bosses"),
                            ("objective", "Objectives"),
                            ("status", "Status effects"),
                            ("ability_grant", "Ability grants"),
                            ("temporary_exit", "Routes"),
                        ] {
                            let mut enabled = model.generated_proposal_kinds.iter().any(|allowed| allowed == kind);
                            if ui.checkbox(&mut enabled, label).changed() {
                                if enabled {
                                    model.generated_proposal_kinds.push(kind.to_owned());
                                    model.generated_proposal_kinds.sort();
                                    model.generated_proposal_kinds.dedup();
                                } else {
                                    model.generated_proposal_kinds.retain(|allowed| allowed != kind);
                                }
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.label("Narration events");
                    ui.small("Choose the event types you want narrated. The cartridge still decides whether an enabled event uses local narration, mock flavor, or plain system text.");
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut model.narration_preferences.summary, "Opening & summaries");
                        ui.checkbox(&mut model.narration_preferences.routine, "Routine actions");
                        ui.checkbox(&mut model.narration_preferences.notable, "Discoveries & movement");
                        ui.checkbox(&mut model.narration_preferences.major, "Major progress");
                        ui.checkbox(&mut model.narration_preferences.critical, "Critical outcomes");
                        ui.checkbox(&mut model.narration_preferences.inspection, "Look & inspect");
                    });

                    ui.add_space(8.0);
                    ui.label("Play contract");
                    egui::ComboBox::from_id_salt("play_contract_preset")
                        .selected_text(model.play_preset.as_str())
                        .show_ui(ui, |ui| {
                            for (value, label) in [
                                ("simulation", "Simulation"),
                                ("rules_first", "Rules First"),
                                ("adventure", "Adventure"),
                                ("chaos", "Chaos"),
                                ("unleashed", "Unleashed"),
                            ] {
                                ui.selectable_value(model.play_preset, value.to_owned(), label);
                            }
                        });
                    for control in &model.play_contract_summary {
                        ui.small(control);
                    }
                    ui.small("Protected: reducer-owned truth, explicit admission, and immutable authored packs.");

                    ui.add_space(8.0);
                    ui.label("Fog of war");
                    egui::ComboBox::from_id_salt("fog_mode_select")
                        .selected_text(model.fog_mode.as_str())
                        .show_ui(ui, |ui| {
                            for label in ["Full", "Known", "Visited"] {
                                ui.selectable_value(model.fog_mode, label.to_owned(), label);
                            }
                        });
                    ui.small("Map visibility rule for this run.");

                    ui.add_space(8.0);
                    ui.label("DM capsule");
                    egui::ComboBox::from_id_salt("dm_capsule_select")
                        .selected_text(model.dm_capsule.as_str())
                        .show_ui(ui, |ui| {
                            for label in [
                                "Hostile Meatgrinder",
                                "Grim Survival",
                                "Slapstick Horror",
                                "Cozy Storybook",
                            ] {
                                ui.selectable_value(model.dm_capsule, label.to_owned(), label);
                            }
                        });
                });
            });

            columns[1].vertical(|ui| {
                ui.group(|ui| {
                    ui.set_max_width(460.0);
                    ui.heading("Narrator Service");
                    ui.label("Bookkeeper / summary helper");
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(model.cpu_helper_model),
                    );
                    ui.small("Reserved for a future CPU-friendly helper lane.");

                    ui.add_space(8.0);
                    ui.label("Backend");
                    egui::ComboBox::from_id_salt("narrator_selector")
                        .selected_text(model.narrator_backend.as_str())
                        .show_ui(ui, |ui| {
                            for label in [
                                "None",
                                "Mock Narrator",
                                "Local llama.cpp / GGUF",
                                "Local OpenAI-compatible",
                                "BYO API Key",
                            ] {
                                ui.selectable_value(
                                    model.narrator_backend,
                                    label.to_owned(),
                                    label,
                                );
                            }
                        });
                    ui.label("Model");
                    egui::ComboBox::from_id_salt("local_narrator_model")
                        .selected_text(model.gpu_narrator_model.as_str())
                        .show_ui(ui, |ui| {
                            for name in &model.local_narrator_models {
                                ui.selectable_value(
                                    model.gpu_narrator_model,
                                    name.clone(),
                                    name,
                                );
                            }
                        });
                    if model.local_narrator_models.is_empty() {
                        ui.small("Drop .gguf files into models/local/, then refresh setup.");
                    }
                    ui.label("Endpoint");
                    ui.add(egui::TextEdit::singleline(model.narrator_endpoint));
                    ui.small("Backend and model remain separate from the DM capsule/persona.");
                });

                ui.add_space(12.0);

                if model.narrator_backend == "BYO API Key" {
                    ui.group(|ui| {
                        ui.heading("Bring Your Own API Key");
                        ui.small("Choose a provider to prefill its endpoint and narrator model. Your key stays only in this running app and is never written to saves or title settings.");
                        let provider_before = model.byo_provider.clone();
                        ui.label("Provider");
                        egui::ComboBox::from_id_salt("byo_provider")
                            .selected_text(model.byo_provider.as_str())
                            .show_ui(ui, |ui| {
                                for provider in ["OpenAI", "xAI", "Anthropic", "Google Gemini", "Custom OpenAI-compatible"] {
                                    ui.selectable_value(model.byo_provider, provider.to_owned(), provider);
                                }
                            });
                        if *model.byo_provider != provider_before {
                            apply_byo_provider_defaults(
                                model.byo_provider,
                                model.byo_model,
                                model.byo_endpoint,
                            );
                        }
                        if ui.button("Apply provider defaults").clicked() {
                            apply_byo_provider_defaults(
                                model.byo_provider,
                                model.byo_model,
                                model.byo_endpoint,
                            );
                        }
                        ui.label("Narrator model");
                        ui.add(egui::TextEdit::singleline(model.byo_model));
                        ui.label("API endpoint");
                        ui.add(egui::TextEdit::singleline(model.byo_endpoint));
                        ui.label("API key");
                        ui.add(egui::TextEdit::singleline(model.byo_api_key).password(true));
                    });
                }

                ui.add_space(12.0);

                ui.group(|ui| {
                    ui.heading("Run Controls");

                    let has_boot_selection =
                        model.selected_record.is_some() || model.selected_invalid.is_some();
                    if ui
                        .add_enabled(has_boot_selection, egui::Button::new("Start Scenario"))
                        .clicked()
                    {
                        action = SetupScreenAction::GenerateGame;
                    }
                    if model.selected_invalid.is_some() {
                        ui.small(
                            "Selected cartridge failed validation. Start Scenario will refuse to boot it.",
                        );
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Load Game").clicked() {
                            action = SetupScreenAction::LoadGame;
                        }
                        if ui.button("Refresh Cartridges").clicked() {
                            action = SetupScreenAction::RefreshCartridges;
                        }
                        ui.add_enabled(false, egui::Button::new("Advanced Generation"));
                    });

                    ui.small("Tabs stay hidden until a run exists, keeping setup uncluttered.");
                });

                ui.add_space(12.0);

                ui.group(|ui| {
                    ui.heading("Project Identity");
                    ui.small("Compact stewardship surface for the tool.");
                    ui.add_space(4.0);

                    egui::CollapsingHeader::new("About Fractal Media Infrastructure")
                        .default_open(false)
                        .show(ui, |ui| {
                            ui.small("Publisher / steward: Fractal Media Infrastructure");
                            ui.small("GitHub: instance001");
                            ui.small("License: GNU Affero General Public License v3.0");
                            ui.small("FMI is a small independent R&D umbrella for open-source AI tooling, cognitive scaffolding experiments, and local-first research systems.");
                            ui.add_space(6.0);
                            ui.hyperlink_to(
                                "Repository: instance001/chatty-quest",
                                "https://github.com/instance001/chatty-quest",
                            );
                            ui.hyperlink_to(
                                "Publisher GitHub: instance001",
                                "https://github.com/instance001",
                            );
                        });
                });
            });
        });

        ui.add_space(16.0);
        show_datapack_status(
            ui,
            model.datapacks,
            model.selected_record.as_ref(),
            model.selected_invalid.as_ref(),
        );
        ui.add_space(10.0);
        ui.label("`v0.1` keeps setup lean on purpose: choose the run shape, then drop straight into the deterministic shell.");
            });
    });

    action
}

pub fn show_splash_screen(ctx: &egui::Context, model: SplashScreenModel<'_>) {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(10, 14, 11))
                .inner_margin(egui::Margin::same(24)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(18.0);
                ui.label(
                    egui::RichText::new(model.title)
                        .size(34.0)
                        .strong()
                        .color(egui::Color32::from_rgb(230, 236, 224)),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(model.subtitle)
                        .size(18.0)
                        .color(egui::Color32::from_rgb(161, 188, 148)),
                );
                if let Some(supporting_line) = model.supporting_line {
                    ui.add_space(6.0);
                    ui.small(supporting_line);
                }
                ui.add_space(18.0);

                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 22, 18))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(68, 81, 66)))
                    .corner_radius(egui::CornerRadius::same(18))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(880.0));
                        if let Some(image_path) = model.image_path {
                            let image_uri = local_file_uri(&image_path);
                            ui.add(
                                egui::Image::from_uri(image_uri)
                                    .max_width(ui.available_width().min(820.0))
                                    .max_height(420.0)
                                    .corner_radius(egui::CornerRadius::same(16)),
                            );
                        } else {
                            ui.add_space(48.0);
                            ui.label(
                                egui::RichText::new("Branding image unavailable")
                                    .size(22.0)
                                    .strong(),
                            );
                            ui.small("The app will continue into setup normally.");
                            ui.add_space(48.0);
                        }
                    });

                ui.add_space(18.0);
                ui.add(
                    egui::ProgressBar::new(model.progress)
                        .desired_width(320.0)
                        .show_percentage(),
                );
                ui.add_space(8.0);
                ui.small(model.footer);
            });
        });
}

pub fn show_game_action_bar(
    ctx: &egui::Context,
    current_run: Option<&RunState>,
    current_bundle: Option<&DatapackBundle>,
    focused_entity_id: Option<&str>,
    current_input: &mut String,
) -> Option<String> {
    let mut queued_command = None;
    let action_bar =
        build_game_action_bar_with_focus(current_run, current_bundle, focused_entity_id);

    egui::TopBottomPanel::bottom("game_action_bar")
        .resizable(false)
        .exact_height(120.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label("Action bar");
            ui.horizontal_wrapped(|ui| {
                for action in &action_bar.primary_actions {
                    if ui.button(&action.label).clicked() {
                        queued_command = Some(action.command.clone());
                    }
                }
            });

            if !action_bar.quick_exits.is_empty() {
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.small("Quick exits:");
                    for exit in &action_bar.quick_exits {
                        if ui.small_button(&exit.destination_name).clicked() {
                            queued_command = Some(format!("go {}", exit.destination_id));
                        }
                    }
                });
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(&action_bar.command_label);
                let response = ui.add(
                    egui::TextEdit::singleline(current_input)
                        .hint_text(&action_bar.command_hint)
                        .desired_width(f32::INFINITY),
                );
                let submit_clicked = ui.button(&action_bar.submit_label).clicked();
                let pressed_enter =
                    response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));

                if submit_clicked || pressed_enter {
                    queued_command = Some(current_input.clone());
                }
            });
        });

    queued_command
}

pub fn show_game_tab(
    ctx: &egui::Context,
    current_run: Option<&RunState>,
    current_bundle: Option<&DatapackBundle>,
    media_panel: &MediaPanelState,
    chaos_mode: f32,
    fog_mode: &str,
    log_lines: &[String],
) -> (Option<String>, Option<AssetViewerRequest>, Option<String>) {
    let mut queued_command = None;
    let mut viewer_request = None;
    let mut focus_entity_id = None;
    let map_layout = current_bundle
        .zip(current_run)
        .map(|(bundle, run)| build_map_layout(bundle, run));

    egui::SidePanel::left("map_panel")
        .resizable(true)
        .default_width(240.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("map_panel_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.heading("Map");
                    if let Some(run) = current_run {
                        if let Some(bundle) = current_bundle {
                            if let Some(sidebar) = build_game_sidebar(run, bundle) {
                                ui.label(format!(
                                    "Current location: {}",
                                    sidebar.current_location_name
                                ));
                                ui.small(&sidebar.current_location_description);
                                if !sidebar.current_location_tags.is_empty() {
                                    ui.small(format!(
                                        "Tags: {}",
                                        sidebar.current_location_tags.join(", ")
                                    ));
                                }
                                ui.small(&sidebar.run_phase_line);
                                ui.small(&sidebar.noise_line);
                                ui.small(&sidebar.noise_hint_line);
                                ui.small(&sidebar.suggested_command_line);
                                if !sidebar.objective_progress_rows.is_empty() {
                                    ui.separator();
                                    ui.label("Objective progress");
                                    for row in &sidebar.objective_progress_rows {
                                        ui.small(row);
                                    }
                                }
                                for status_line in &sidebar.current_location_status_lines {
                                    ui.small(status_line);
                                }
                                if let Some(layout) = &map_layout {
                                    ui.separator();
                                    ui.label("Map tiles");
                                    render_map_layout(
                                        ui,
                                        layout,
                                        run,
                                        fog_mode,
                                        &mut queued_command,
                                        &mut viewer_request,
                                    );
                                }
                                if !sidebar.connected_exits.is_empty() {
                                    ui.separator();
                                    ui.label("Connected exits");
                                    for exit in &sidebar.connected_exits {
                                        if ui
                                            .button(format!(
                                                "Go to {}{}",
                                                exit.destination_name,
                                                location_state_suffix(
                                                    exit.locked,
                                                    exit.broken,
                                                    exit.barricaded
                                                )
                                            ))
                                            .clicked()
                                        {
                                            queued_command =
                                                Some(format!("go {}", exit.destination_id));
                                        }
                                        if let Some(route_note) = &exit.route_note {
                                            ui.small(format!("  {}", route_note));
                                        }
                                        if let Some(threat_forecast) = &exit.threat_forecast {
                                            ui.small(format!("  forecast: {}", threat_forecast));
                                        }
                                    }
                                }
                                if sidebar.local_item_count > 0 {
                                    ui.separator();
                                    ui.label("Items here");
                                    for item in build_local_item_action_rows(run, bundle) {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.small(&item.item_name);
                                            if ui.small_button("Inspect").clicked() {
                                                queued_command = Some(item.inspect_command.clone());
                                            }
                                            if ui.small_button("Take").clicked() {
                                                queued_command = Some(item.take_command.clone());
                                            }
                                        });
                                    }
                                }
                            }
                            ui.separator();
                            ui.label("Known locations");
                            if let Some(sidebar) = build_game_sidebar(run, bundle) {
                                for location in &sidebar.known_locations {
                                    ui.small(format!(
                                        "{} {}{}",
                                        location.marker,
                                        location.location_name,
                                        location_state_suffix(
                                            location.locked,
                                            location.broken,
                                            location.barricaded
                                        )
                                    ));
                                }
                            }
                        }
                    } else {
                        ui.label("No active run.");
                    }
                });
        });

    egui::SidePanel::right("media_panel")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let display = build_media_panel_display(media_panel);
                    ui.heading("Media");
                    if current_bundle.is_none() {
                        ui.label("No active run.");
                        return;
                    }

            ui.label("Event-aware media placeholder panel");
            ui.separator();
            ui.label(egui::RichText::new(&media_panel.title).strong());
            ui.small(&media_panel.subtitle);
            if let Some(line) = &display.role_line {
                ui.small(line);
            }
            ui.add_space(6.0);
            ui.small(&display.media_state_line);
            if let Some(line) = &display.image_source_line {
                ui.small(line);
            }
            ui.small(&display.placeholder_message);
            ui.add_space(6.0);
            show_media_panel_preview(ui, media_panel, &mut viewer_request);
            ui.add_space(6.0);
            show_media_slot(ui, "Image", media_panel.selected_image.as_ref());
            show_media_slot(ui, "Motion", media_panel.selected_motion.as_ref());
            show_media_slot(ui, "Audio", media_panel.selected_audio.as_ref());
            if ui.button("Open Focus Viewer").clicked() {
                viewer_request = Some(build_media_panel_asset_viewer_request(media_panel));
            }
            if let Some(line) = &display.narrator_brief_line {
                ui.add_space(6.0);
                ui.small(line);
            }

            if let Some(line) = &display.current_location_name_line {
                ui.add_space(6.0);
                ui.small(line);
            }
            if let Some(line) = &display.current_location_description_line {
                ui.small(line);
            }
            if let Some(line) = &display.world_tone_line {
                ui.add_space(6.0);
                ui.small(line);
            }
            if let Some(line) = &display.boundary_rule_line {
                ui.small(line);
            }

            if !display.active_cue_rows.is_empty() {
                ui.separator();
                ui.label("Active cues");
                for row in &display.active_cue_rows {
                    ui.small(row);
                }
            }

            if !media_panel.observed_entity_rows.is_empty() {
                ui.separator();
                ui.label("Observable entities");
                ui.small("Host-confirmed entities at this location. Focus changes presentation only; it never issues an action or changes state.");
                for entity in &media_panel.observed_entities {
                    ui.horizontal_wrapped(|ui| {
                        ui.small(&entity.label);
                        let label = if media_panel.focused_entity_id.as_deref()
                            == Some(entity.id.as_str())
                        {
                            "Focused"
                        } else {
                            "Focus"
                        };
                        if ui.small_button(label).clicked() {
                            focus_entity_id = Some(entity.id.clone());
                        }
                    });
                    ui.small(format!("  {}", entity.detail));
                }
            }

            if !display.future_hook_rows.is_empty() {
                ui.separator();
                ui.label("Future media hooks");
                for row in &display.future_hook_rows {
                    ui.small(row);
                }
            }

            if !display.encounter_snapshot_rows.is_empty() {
                ui.separator();
                ui.label("Encounter snapshot");
                for line in &display.encounter_snapshot_rows {
                    ui.small(line);
                }
            }

                    ui.small("Future datapack image and video hooks can bind to these event-derived cues.");
                });
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        let header_chips = build_game_header_chips(current_run, chaos_mode);
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                for chip in &header_chips {
                    status_chip(ui, &chip.label, &chip.value);
                }
            });
        });

        if let Some(outcome) = build_outcome_banner(current_run) {
            ui.add_space(8.0);
            ui.group(|ui| {
                let tone = if outcome.label == "WIN" {
                    egui::Color32::from_rgb(152, 214, 136)
                } else {
                    egui::Color32::from_rgb(214, 110, 110)
                };
                ui.visuals_mut().override_text_color = Some(tone);
                ui.label(egui::RichText::new(&outcome.label).strong().size(18.0));
                ui.small(&outcome.detail);
                if let Some(followup) = &outcome.followup {
                    ui.small(followup);
                }
                ui.visuals_mut().override_text_color = None;
            });
        }

        ui.add_space(10.0);
        ui.heading("Game Log");
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in log_lines {
                    ui.label(line);
                }
            });
    });

    (queued_command, viewer_request, focus_entity_id)
}

pub fn show_inventory_tab(
    ctx: &egui::Context,
    current_run: Option<&RunState>,
    current_bundle: Option<&DatapackBundle>,
) -> (Option<String>, Option<AssetViewerRequest>) {
    let mut queued_command = None;
    let mut viewer_request = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Inventory");
        if let (Some(run), Some(bundle)) = (current_run, current_bundle) {
            ui.label("Deterministic inventory state");
            ui.separator();
            let inventory_rows = build_inventory_action_rows(run, Some(bundle));
            for row in inventory_rows {
                let item = run
                    .controlled_inventory()
                    .and_then(|inventory| inventory.iter().find(|item| item.id == row.item_id));
                let template = bundle
                    .items
                    .iter()
                    .find(|candidate| candidate.id == row.item_id);
                ui.horizontal(|ui| {
                    if let (Some(template), Some(item)) = (template, item) {
                        show_inventory_thumbnail(ui, bundle, template, item, &mut viewer_request);
                    }

                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(&row.display_name);

                            if ui.small_button("Inspect").clicked() {
                                queued_command = Some(row.inspect_command.clone());
                            }

                            if let Some(equip_command) = &row.equip_command
                                && ui.small_button("Equip").clicked()
                            {
                                queued_command = Some(equip_command.clone());
                            }

                            if ui.small_button("Use").clicked() {
                                queued_command = Some(row.use_command.clone());
                            }

                            if ui.small_button("Drop").clicked() {
                                queued_command = Some(format!("drop {}", row.item_id));
                            }

                            if row.show_view_button
                                && ui.small_button("View").clicked()
                                && let (Some(template), Some(item)) = (template, item)
                            {
                                viewer_request =
                                    Some(build_item_asset_viewer_request(bundle, item, template));
                            }
                        });
                        ui.small(&row.description_line);
                        if let Some(actor_contract_line) = &row.actor_contract_line {
                            ui.small(actor_contract_line);
                        }
                        if let Some(template_warning) = &row.template_warning {
                            ui.small(template_warning);
                        }
                        let grants = run
                            .ability_grants
                            .iter()
                            .filter(|grant| {
                                matches!(
                                    &grant.owner,
                                    crate::game::state::ItemOwner::Entity(owner_id)
                                        if item.is_some_and(|item| owner_id == &item.instance_id)
                                )
                            })
                            .collect::<Vec<_>>();
                        if !grants.is_empty() {
                            ui.small("Attached ability grants:");
                            for grant in &grants {
                                let name = bundle
                                    .abilities
                                    .iter()
                                    .find(|ability| ability.id == grant.template_id)
                                    .map(|ability| ability.name.as_str())
                                    .unwrap_or(grant.template_id.as_str());
                                let uses = grant.remaining_uses.map_or_else(
                                    || "infinite uses".to_owned(),
                                    |uses| format!("{uses} uses"),
                                );
                                let expiry = grant.expires_at_turn.map_or_else(
                                    || "permanent".to_owned(),
                                    |turn| format!("expires in {} turns", turn.saturating_sub(run.turn_index)),
                                );
                                ui.small(format!(
                                    "  {name}: {uses} | {:?} | {expiry}",
                                    grant.source
                                ));
                            }
                            let study_grants = grants
                                .iter()
                                .filter(|grant| {
                                    grant.source == crate::game::state::AbilityGrantSource::Item
                                        && bundle
                                            .abilities
                                            .iter()
                                            .find(|template| template.id == grant.template_id)
                                            .is_some_and(|template| template.scroll_learnable)
                                })
                                .collect::<Vec<_>>();
                            if let [grant] = study_grants.as_slice() {
                                let template = bundle
                                    .abilities
                                    .iter()
                                    .find(|template| template.id == grant.template_id)
                                    .expect("filtered study grant has an authored template");
                                if crate::game::owner_can_study_scroll_template(
                                    run,
                                    bundle,
                                    &run.controlled_inventory_owner(),
                                    &template.id,
                                ) && !crate::game::owner_has_permanent_ability(
                                    run,
                                    &run.controlled_inventory_owner(),
                                    &template.id,
                                ) {
                                    if ui.small_button("Study").clicked() {
                                        queued_command = Some(format!("learn {}", row.item_id));
                                    }
                                } else {
                                    ui.small(format!(
                                        "Study {}: requires matching permanent literacy.",
                                        template.name
                                    ));
                                }
                            }
                        }
                    });
                });
                ui.add_space(6.0);
            }
        } else {
            ui.label("No active run.");
        }
        ui.small("Future scenario-specific tabs should be added beside Inventory, not folded into the Game tab.");
    });

    (queued_command, viewer_request)
}

pub fn show_asset_viewer(ctx: &egui::Context, request: &AssetViewerRequest) -> bool {
    let mut open = true;
    let mut should_close = false;
    let chrome = build_asset_viewer_chrome();

    egui::Window::new(request.title.as_str())
        .id(egui::Id::new(format!("asset_viewer:{}", request.viewer_id)))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(560.0)
        .default_height(640.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(&request.title).strong().size(20.0));
            if let Some(subtitle) = &request.subtitle {
                ui.small(subtitle);
            }
            ui.small(format!("Source kind: {}", request.source_kind));
            if let Some(source_label) = &request.resolved_source_label {
                ui.small(format!("Resolved source: {}", source_label));
            }

            ui.add_space(8.0);

            if let Some(image_path) = &request.image_path {
                if request.using_engine_fallback {
                    show_engine_fallback_card(ui);
                } else {
                    let image_uri = local_file_uri(image_path);
                    ui.add(
                        egui::Image::from_uri(image_uri)
                            .max_width(480.0)
                            .max_height(480.0)
                            .maintain_aspect_ratio(true),
                    );
                    ui.small(format!("Image path: {}", image_path));
                }
            } else {
                show_engine_fallback_card(ui);
                ui.small(&chrome.missing_image_line);
            }

            if request.using_datapack_fallback {
                ui.small(&chrome.datapack_fallback_line);
            } else if request.using_engine_fallback {
                ui.small(&chrome.engine_fallback_line);
            }

            if let Some(description) = &request.description {
                ui.add_space(8.0);
                ui.label(description);
            }

            ui.add_space(10.0);
            if ui.button(&chrome.close_label).clicked() {
                should_close = true;
            }
        });

    open && !should_close
}

pub fn show_library_tab(
    ctx: &egui::Context,
    datapack_id: Option<&str>,
    state: &mut LibraryUiState,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Player Library");
        ui.small("Player-owned templates and tagged media; authored datapacks remain unchanged.");
        if let Some(feedback) = &state.feedback {
            ui.colored_label(
                if feedback.is_error { egui::Color32::LIGHT_RED } else { egui::Color32::LIGHT_GREEN },
                &feedback.message,
            );
        }
        let Some(datapack_id) = datapack_id else {
            ui.label("Start a run to view compatible entries.");
            return;
        };
        match crate::collection::load_collection_items_for_datapack(datapack_id) {
            Ok(items) if items.is_empty() => {
                ui.label("No saved narrator templates for this datapack.");
            }
            Ok(items) => {
                for item in items {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(&item.name).strong());
                        ui.small(&item.description);
                        ui.small(player_template_library_summary(&item));
                        if let Some(capability_summary) = player_template_capability_summary(&item)
                        {
                            ui.small(capability_summary);
                        }
                        if let Some(media_summary) = player_template_media_summary(&item) {
                            ui.small(media_summary);
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .button(if item.enabled { "Disable" } else { "Enable" })
                                .clicked()
                            {
                                state.feedback = action_feedback(crate::collection::set_collection_item_enabled(
                                    &item.id,
                                    !item.enabled,
                                ), if item.enabled { "Template disabled." } else { "Template enabled." });
                                ctx.request_repaint();
                            }
                            if ui.button("Edit tags").clicked() {
                                state.editing_tags = Some(LibraryTagEdit {
                                    target: LibraryTagTarget::Template(item.id.clone()),
                                    text: item.tags.join(", "),
                                });
                            }
                            if ui.button("Edit media").clicked() {
                                state.editing_template_media = Some(LibraryTemplateMediaEdit {
                                    template_id: item.id.clone(),
                                    image: item
                                        .media_references
                                        .image
                                        .clone()
                                        .unwrap_or_default(),
                                    motion: item
                                        .media_references
                                        .motion
                                        .clone()
                                        .unwrap_or_default(),
                                    audio: item
                                        .media_references
                                        .audio
                                        .clone()
                                        .unwrap_or_default(),
                                });
                            }
                            if ui.button("Remove").clicked() {
                                state.pending_removal = Some(LibraryRemovalTarget::Template {
                                    id: item.id.clone(),
                                    name: item.name.clone(),
                                });
                            }
                        });
                    });
                }
            }
            Err(error) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        }
        ui.separator();
        ui.label("Tagged media pool");
        ui.small("Import a local image, GIF/video, or audio file. The library stores only its path and tags.");
        ui.horizontal(|ui| {
            ui.label("File");
            ui.add(egui::TextEdit::singleline(&mut state.import_path).hint_text("C:\\media\\artifact.png"));
        });
        ui.horizontal(|ui| {
            ui.label("Tags");
            ui.add(egui::TextEdit::singleline(&mut state.import_tags).hint_text("weapon, neon"));
            if ui.button("Import local file").clicked() {
                let result = crate::media_pool::add_media_asset(
                    state.import_path.clone(),
                    comma_tags(&state.import_tags),
                );
                let succeeded = result.is_ok();
                state.feedback = action_feedback(result, "Media asset imported.");
                if succeeded {
                    state.import_path.clear();
                    state.import_tags.clear();
                }
                ctx.request_repaint();
            }
        });
        match crate::media_pool::load_player_media_pool() {
            Ok(pool) if pool.assets.is_empty() => {
                ui.small("No player media assets configured yet.");
            }
            Ok(pool) => {
                for asset in pool.assets {
                    ui.horizontal_wrapped(|ui| {
                        let file_present = std::path::Path::new(&asset.path).is_file();
                        if asset.kind == crate::media_pool::PlayerMediaKind::Image && file_present {
                            ui.add(
                                egui::Image::from_uri(local_file_uri(&asset.path))
                                    .fit_to_exact_size(egui::vec2(48.0, 48.0)),
                            );
                        }
                        ui.small(format!(
                            "{:?}: {} [{}] | {} | {}",
                            asset.kind,
                            asset.path,
                            asset.tags.join(", "),
                            if asset.enabled { "enabled" } else { "disabled" },
                            if file_present { "file present" } else { "MISSING FILE" }
                        ));
                        if ui
                            .button(if asset.enabled { "Disable" } else { "Enable" })
                            .clicked()
                        {
                            state.feedback = action_feedback(crate::media_pool::set_media_asset_enabled(
                                &asset.path,
                                !asset.enabled,
                            ), if asset.enabled { "Media asset disabled." } else { "Media asset enabled." });
                            ctx.request_repaint();
                        }
                        if ui.button("Edit tags").clicked() {
                            state.editing_tags = Some(LibraryTagEdit {
                                target: LibraryTagTarget::Media(asset.path.clone()),
                                text: asset.tags.join(", "),
                            });
                        }
                        if ui.button("Remove").clicked() {
                            state.pending_removal = Some(LibraryRemovalTarget::Media {
                                path: asset.path.clone(),
                            });
                        }
                    });
                }
            }
            Err(error) => {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        }

        show_library_tag_editor(ctx, state);
        show_library_template_media_editor(ctx, state);
        show_library_remove_confirmation(ctx, state);
    });
}

fn player_template_library_summary(template: &PlayerTemplate) -> String {
    let kind = match template.kind {
        PlayerTemplateKind::Item => "Item",
        PlayerTemplateKind::Weapon => "Weapon",
        PlayerTemplateKind::Status => "Status",
        PlayerTemplateKind::Effect => "Effect",
        PlayerTemplateKind::Npc => "NPC",
        PlayerTemplateKind::Enemy => "Enemy",
        PlayerTemplateKind::Boss => "Boss",
        PlayerTemplateKind::Objective => "Objective",
        PlayerTemplateKind::Location => "Location",
        PlayerTemplateKind::TemporaryExit => "Temporary exit",
    };
    let mechanics = match template.effective_mechanics() {
        PlayerTemplateMechanics::Item { damage, condition } => match condition {
            Some(condition) => format!("damage {damage} | condition: {condition}"),
            None => format!("damage {damage}"),
        },
        PlayerTemplateMechanics::Status {
            trigger,
            duration,
            additive_damage,
            additive_healing,
            ..
        } => format!(
            "trigger: {trigger} | duration: {duration} | damage {additive_damage:+} | healing {additive_healing:+}"
        ),
        PlayerTemplateMechanics::Npc { faction } => format!("faction: {faction}"),
        PlayerTemplateMechanics::Enemy { max_hp, damage }
        | PlayerTemplateMechanics::Boss { max_hp, damage } => {
            format!("HP {max_hp} | damage {damage}")
        }
        PlayerTemplateMechanics::Objective { completion_key } => {
            format!("completion key: {completion_key}")
        }
        PlayerTemplateMechanics::Location {
            location_id,
            initially_known,
        } => format!(
            "location: {location_id} | {}",
            if initially_known { "known" } else { "hidden" }
        ),
        PlayerTemplateMechanics::TemporaryExit {
            from_location_id,
            to_location_id,
            locked,
        } => format!(
            "{from_location_id} -> {to_location_id} | {}",
            if locked { "locked" } else { "open" }
        ),
    };
    format!(
        "{kind} | {mechanics} | Tags: {} | {}",
        if template.tags.is_empty() {
            "none".to_owned()
        } else {
            template.tags.join(", ")
        },
        if template.enabled {
            "enabled"
        } else {
            "disabled"
        }
    )
}

fn player_template_capability_summary(template: &PlayerTemplate) -> Option<String> {
    let actor = template.actor.as_ref()?;
    let capabilities = if actor.capabilities.is_empty() {
        "none".to_owned()
    } else {
        actor.capabilities.join(", ")
    };
    Some(format!(
        "Actor contract: {capabilities} | disposition: {:?}",
        template.disposition
    ))
}

fn player_template_media_summary(template: &PlayerTemplate) -> Option<String> {
    let mut kinds = Vec::new();
    if template.media_references.image.is_some() {
        kinds.push("image");
    }
    if template.media_references.motion.is_some() {
        kinds.push("motion");
    }
    if template.media_references.audio.is_some() {
        kinds.push("audio");
    }
    (!kinds.is_empty()).then(|| format!("Exact media: {}", kinds.join(", ")))
}

fn comma_tags(value: &str) -> Vec<String> {
    value.split(',').map(str::to_owned).collect()
}

fn action_feedback(result: Result<(), String>, success: &str) -> Option<LibraryFeedback> {
    Some(match result {
        Ok(()) => LibraryFeedback {
            message: success.to_owned(),
            is_error: false,
        },
        Err(error) => LibraryFeedback {
            message: format!("Library update failed: {error}"),
            is_error: true,
        },
    })
}

fn show_library_tag_editor(ctx: &egui::Context, state: &mut LibraryUiState) {
    let Some(editor) = state.editing_tags.as_mut() else {
        return;
    };
    let mut close = false;
    egui::Window::new("Edit library tags")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Comma-separated tags");
            ui.add(egui::TextEdit::singleline(&mut editor.text).desired_width(320.0));
            ui.horizontal(|ui| {
                if ui.button("Save tags").clicked() {
                    let result = match &editor.target {
                        LibraryTagTarget::Template(id) => {
                            crate::collection::set_collection_item_tags(
                                id,
                                comma_tags(&editor.text),
                            )
                        }
                        LibraryTagTarget::Media(path) => {
                            crate::media_pool::set_media_asset_tags(path, comma_tags(&editor.text))
                        }
                    };
                    state.feedback = action_feedback(result, "Tags saved.");
                    close = true;
                    ctx.request_repaint();
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    if close {
        state.editing_tags = None;
    }
}

fn show_library_template_media_editor(ctx: &egui::Context, state: &mut LibraryUiState) {
    let Some(editor) = state.editing_template_media.as_mut() else {
        return;
    };
    let mut close = false;
    egui::Window::new("Edit template media")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.small(
                "Optional exact local paths. A configured path takes priority over tagged pool media; leave a field blank to use the pool.",
            );
            ui.label("Image");
            ui.add(egui::TextEdit::singleline(&mut editor.image).desired_width(440.0));
            ui.label("GIF/video");
            ui.add(egui::TextEdit::singleline(&mut editor.motion).desired_width(440.0));
            ui.label("Audio");
            ui.add(egui::TextEdit::singleline(&mut editor.audio).desired_width(440.0));
            ui.horizontal(|ui| {
                if ui.button("Save media").clicked() {
                    state.feedback = action_feedback(
                        crate::collection::set_collection_item_media_references(
                            &editor.template_id,
                            crate::collection::PlayerTemplateMediaReferences {
                                image: Some(editor.image.clone()),
                                motion: Some(editor.motion.clone()),
                                audio: Some(editor.audio.clone()),
                            },
                        ),
                        "Template media saved.",
                    );
                    close = true;
                    ctx.request_repaint();
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    if close {
        state.editing_template_media = None;
    }
}

fn show_library_remove_confirmation(ctx: &egui::Context, state: &mut LibraryUiState) {
    let Some(target) = state.pending_removal.as_ref() else {
        return;
    };
    let label = match target {
        LibraryRemovalTarget::Template { name, .. } => format!("template '{name}'"),
        LibraryRemovalTarget::Media { path } => format!("media asset '{path}'"),
    };
    let mut remove = false;
    let mut cancel = false;
    egui::Window::new("Remove from player library?")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(format!("Remove {label}? This cannot be undone."));
            ui.horizontal(|ui| {
                if ui.button("Remove").clicked() {
                    remove = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if remove {
        let result = match state
            .pending_removal
            .as_ref()
            .expect("removal target exists")
        {
            LibraryRemovalTarget::Template { id, .. } => {
                crate::collection::remove_collection_item(id)
            }
            LibraryRemovalTarget::Media { path } => crate::media_pool::remove_media_asset(path),
        };
        state.feedback = action_feedback(result, "Library entry removed.");
        state.pending_removal = None;
        ctx.request_repaint();
    } else if cancel {
        state.pending_removal = None;
    }
}

pub fn show_character_tab(
    ctx: &egui::Context,
    current_run: Option<&RunState>,
    current_bundle: Option<&DatapackBundle>,
) -> (Option<String>, Option<AssetViewerRequest>) {
    let mut queued_command = None;
    let mut viewer_request = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Character");
        if let Some(run) = current_run {
            let summary = build_character_summary(run, current_bundle);
            let actions = build_character_actions(&summary);
            ui.label("Character stats");
            ui.separator();
            ui.label(format!("Datapack id: {}", summary.datapack_id));
            ui.label(format!("Run phase: {}", summary.run_phase));
            ui.label(format!("HP: {} / {}", summary.hp, summary.max_hp));
            ui.label(format!(
                "Noise: {} ({})",
                summary.noise_level, summary.noise_label
            ));
            ui.label(format!(
                "Current location id: {}",
                summary.current_location_id
            ));
            if let Some(location_name) = &summary.current_location_name {
                ui.small(format!("Current location name: {}", location_name));
            }
            ui.label(format!(
                "Objective complete: {}",
                if summary.objective_complete {
                    "Yes"
                } else {
                    "No"
                }
            ));
            ui.label(format!("Active objective id: {}", summary.active_objective_id));
            ui.small(&summary.active_objective_description);
            for row in &summary.objective_condition_rows {
                ui.small(row);
            }
            ui.separator();
            ui.label("Abilities and progression");
            let controlled_actor = run.runtime_actor(&run.player_entity_id);
            let player_tags = controlled_actor
                .as_ref()
                .map(|actor| actor.tags.clone())
                .unwrap_or_else(|| run.player_tags.clone());
            let player_tags = if player_tags.is_empty() {
                "none".to_owned()
            } else {
                player_tags.join(", ")
            };
            ui.small(format!("Access tags: {player_tags}"));
            let progress_pools = run.progress_pools_for_actor(&run.player_entity_id);
            if progress_pools.is_empty() {
                ui.small("Progress: none yet");
            } else {
                let mut sorted_progress_pools = progress_pools.iter().collect::<Vec<_>>();
                sorted_progress_pools.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
                for (pool, total) in sorted_progress_pools {
                    let level = current_bundle
                        .map(|bundle| crate::game::skill_level_for_progress(bundle, pool, *total))
                        .unwrap_or_default();
                    ui.small(format!("{pool} XP: {total} | level {level}"));
                }
            }
            let player_id = &run.player_entity_id;
            let grants = run
                .ability_grants
                .iter()
                .filter(|grant| run.ability_grant_is_accessible_to(grant, player_id))
                .collect::<Vec<_>>();
            if grants.is_empty() {
                ui.small("Abilities: none available");
            } else {
                for grant in grants {
                    let usable = current_bundle.is_some_and(|bundle| {
                        crate::game::ability_grant_is_usable_by(run, bundle, grant, player_id)
                    });
                    let template = current_bundle.and_then(|bundle| {
                        bundle
                            .abilities
                            .iter()
                            .find(|template| template.id == grant.template_id)
                    });
                    let name = template.map_or(grant.template_id.as_str(), |template| template.name.as_str());
                    let uses = grant
                        .remaining_uses
                        .map_or("infinite".to_owned(), |uses| uses.to_string());
                    let owner = match &grant.owner {
                        crate::game::state::ItemOwner::Player => "you".to_owned(),
                        crate::game::state::ItemOwner::Entity(id) => format!("carried by {id}"),
                        crate::game::state::ItemOwner::Location(id) => format!("at {id}"),
                    };
                    let expiry = grant.expires_at_turn.map_or_else(
                        || "permanent".to_owned(),
                        |turn| format!("expires in {} turns", turn.saturating_sub(run.turn_index)),
                    );
                    let cooldown = crate::game::ability_grant_cooldown_remaining(run, grant);
                    let cooldown = if cooldown == 0 {
                        "ready".to_owned()
                    } else {
                        format!("cooldown: {cooldown} world turn(s)")
                    };
                    ui.small(format!(
                        "{name}: {uses} uses | {owner} | {:?} | {expiry} | {cooldown}",
                        grant.source,
                    ));
                    if let Some(template) = template {
                        if usable {
                            ui.small(format!("Cast: `cast {} at <target>`", template.id));
                        } else {
                            let reason = current_bundle
                                .and_then(|bundle| {
                                    crate::game::ability_grant_unavailability_reason(
                                        run, bundle, grant, player_id,
                                    )
                                })
                                .unwrap_or_else(|| "ability data is unavailable".to_owned());
                            ui.small(format!("Unavailable: {reason}."));
                        }
                    }
                }
            }
            if let Some(bundle) = current_bundle {
                let owner = run.controlled_inventory_owner();
                let actor_tags = controlled_actor
                    .as_ref()
                    .map(|actor| actor.tags.as_slice())
                    .unwrap_or(&run.player_tags);
                let unlocks = bundle
                    .abilities
                    .iter()
                    .filter(|template| {
                        crate::game::owner_has_unlock_choice_for_template(
                            run,
                            bundle,
                            &owner,
                            &template.id,
                        )
                            && template.required_tags.iter().all(|tag| {
                                actor_tags
                                    .iter()
                                    .any(|owned| owned.eq_ignore_ascii_case(tag))
                            })
                            && !crate::game::owner_has_permanent_ability(
                                run,
                                &owner,
                                &template.id,
                            )
                    })
                    .collect::<Vec<_>>();
                if !unlocks.is_empty() {
                    ui.small("Available unlocks:");
                    for template in unlocks {
                        ui.horizontal(|ui| {
                            ui.small(format!(
                                "{} ({} {} XP)",
                                template.name, template.required_xp, template.progress_pool
                            ));
                            if ui.button("Learn").clicked() {
                                queued_command = Some(format!("learn {}", template.id));
                            }
                        });
                    }
                }
            }
            if !summary.utility_relevance_rows.is_empty() {
                ui.separator();
                ui.label("Utility relevance");
                for row in &summary.utility_relevance_rows {
                    ui.small(row);
                }
            }
            if !summary.security_summary_rows.is_empty() {
                ui.separator();
                ui.label("Siege security");
                for row in &summary.security_summary_rows {
                    ui.small(row);
                }
            }
            if !summary.attractor_summary_rows.is_empty() {
                ui.separator();
                ui.label("Active attractors");
                for row in &summary.attractor_summary_rows {
                    ui.small(row);
                }
            }
            ui.small(format!(
                "Enemies defeated: {} | Bosses defeated: {}",
                summary.enemies_defeated, summary.bosses_defeated
            ));
            ui.small(format!(
                "Locked locations: {}",
                if summary.locked_locations.is_empty() {
                    "none".to_owned()
                } else {
                    summary.locked_locations.join(", ")
                }
            ));
            ui.small(format!(
                "Broken gates: {}",
                if summary.broken_locked_locations.is_empty() {
                    "none".to_owned()
                } else {
                    summary.broken_locked_locations.join(", ")
                }
            ));
            ui.small(format!(
                "Barricaded locations: {}",
                if summary.barricaded_locations.is_empty() {
                    "none".to_owned()
                } else {
                    summary.barricaded_locations.join(", ")
                }
            ));
            if !summary.rolling_summary.is_empty() {
                ui.separator();
                ui.label("Rolling summary");
                for line in &summary.rolling_summary {
                    ui.small(line);
                }
            }
            if let Some(bundle) = current_bundle {
                ui.separator();
                if let Some(pack_folder_line) = &actions.pack_folder_line {
                    ui.label(pack_folder_line);
                }
                if let Some(objective_tags_line) = &actions.objective_tags_line {
                    ui.small(objective_tags_line);
                }

                if let Some(location) = bundle
                    .locations
                    .iter()
                    .find(|location| location.id == run.current_location_id)
                    && let Some(view_current_location_label) = &actions.view_current_location_label
                    && ui.button(view_current_location_label).clicked()
                {
                    viewer_request =
                        Some(build_location_asset_viewer_request(bundle, run, location));
                }

                if let Some(equipped_item_id) = &summary.equipped_item_id
                    && let Some(inventory_item) =
                        run.entity_inventory(&run.controlled_inventory_owner())
                            .and_then(|inventory| {
                                inventory.iter().find(|item| item.id == *equipped_item_id)
                            })
                    && let Some(template) = bundle
                        .items
                        .iter()
                        .find(|candidate| candidate.id == *equipped_item_id)
                    && let Some(view_equipped_item_label) = &actions.view_equipped_item_label
                    && ui.button(view_equipped_item_label).clicked()
                {
                    viewer_request =
                        Some(build_item_asset_viewer_request(bundle, inventory_item, template));
                }
            }
        } else {
            ui.label("No active run.");
        }
        ui.small("This tab is where future skills, sanity, mana, or other scenario-specific stats can expand cleanly.");
    });

    (queued_command, viewer_request)
}

pub fn show_diagnostics_tab(ctx: &egui::Context, report: &DiagnosticReport) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let summary = build_diagnostics_summary(report);
        ui.heading("Diagnostics");
        ui.label("Application healthcheck and canonical event stream.");

        ui.separator();
        ui.label("Application");
        ui.small(format!("Narrator attached: {}", summary.narrator_attached));
        ui.small(format!("Save path: {}", summary.save_path));
        ui.small(format!("Save version: {}", summary.current_save_version));
        ui.small(format!(
            "Datapack schema: {}",
            summary.datapack_schema_version
        ));
        ui.small(format!("Loaded run: {}", summary.loaded_run));
        ui.small(format!(
            "Loaded save version: {}",
            summary.loaded_save_version
        ));

        ui.separator();
        ui.label("Content");
        ui.small(format!("Valid datapacks: {}", summary.valid_datapacks));
        ui.small(format!(
            "Invalid datapacks: {}",
            summary.invalid_datapack_count
        ));
        ui.small(format!("Active bundle: {}", summary.active_bundle_name));
        if let Some(line) = &summary.template_counts {
            ui.small(line);
        }
        if let Some(line) = &summary.presentation_coverage {
            ui.small(line);
        }
        if let Some(line) = &summary.media_assets {
            ui.small(line);
        }

        if !summary.invalid_datapack_rows.is_empty() {
            ui.separator();
            ui.label("Invalid datapacks");
            for invalid in &summary.invalid_datapack_rows {
                let label = invalid
                    .display_name
                    .as_deref()
                    .unwrap_or(invalid.folder_name.as_str());
                let version = invalid
                    .version
                    .as_deref()
                    .map(|version| format!(" v{}", version))
                    .unwrap_or_default();
                ui.small(format!("{}{} ({})", label, version, invalid.folder_name));
                for error in &invalid.errors {
                    ui.small(format!("- {}", error));
                }
            }
        }

        if summary.media_missing_summary.is_some() {
            ui.separator();
            ui.label("Media asset checks");
            if let Some(line) = &summary.media_missing_summary {
                ui.small(line);
            }
            if !summary.media_missing_rows.is_empty() {
                egui::ScrollArea::vertical()
                    .id_salt("diagnostics_media_asset_checks")
                    .max_height(160.0)
                    .show(ui, |ui| {
                        for entry in &summary.media_missing_rows {
                            ui.small(&entry.summary);
                            ui.small(&entry.expected_path);
                        }
                    });
            }
        }

        ui.separator();
        ui.label("Player library health");
        if report.player_library_report.template_rows.is_empty()
            && report.player_library_report.media_rows.is_empty()
            && report.player_library_report.errors.is_empty()
        {
            ui.small("No compatible player templates or player media entries.");
        }
        for row in &report.player_library_report.template_rows {
            ui.small(row);
        }
        for row in &report.player_library_report.media_rows {
            let color = if row.contains("MISSING FILE") {
                egui::Color32::LIGHT_RED
            } else {
                ui.visuals().text_color()
            };
            ui.colored_label(color, row);
        }
        for error in &report.player_library_report.errors {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }

        ui.separator();
        ui.label("Run health");
        if summary.run_health_rows.is_empty() {
            ui.small("No active run.");
        } else {
            for row in &summary.run_health_rows {
                ui.small(row);
            }
        }

        ui.separator();
        ui.label("Environment checks");
        for row in &summary.environment_rows {
            ui.small(row);
        }

        ui.separator();
        ui.label("Warnings");
        for row in &summary.warning_rows {
            ui.small(row);
        }

        ui.separator();
        ui.label("Recent events");
        ui.small(&summary.event_counters);
        if summary.recent_events.is_empty() {
            ui.small("No reducer events recorded yet.");
        } else {
            egui::ScrollArea::vertical()
                .id_salt("diagnostics_recent_events")
                .show(ui, |ui| {
                    for event in &summary.recent_events {
                        ui.small(event);
                    }
                });
        }
    });
}

fn status_chip(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.group(|ui| {
        ui.label(egui::RichText::new(label).strong());
        ui.small(value);
    });
}

fn badge_chip(ui: &mut egui::Ui, label: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.22))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.7)))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(4, 2))
        .show(ui, |ui| {
            ui.small(egui::RichText::new(label).color(color));
        });
}

fn location_state_suffix(locked: bool, broken: bool, barricaded: bool) -> &'static str {
    if locked {
        " [locked]"
    } else if broken {
        " [broken]"
    } else if barricaded {
        " [barricaded]"
    } else {
        ""
    }
}

fn show_media_slot(ui: &mut egui::Ui, label: &str, asset: Option<&crate::media::MediaAssetStatus>) {
    match asset {
        Some(asset) if asset.present => {
            ui.small(format!("{}: ready ({})", label, asset.relative_path));
        }
        Some(asset) => {
            ui.small(format!("{}: missing ({})", label, asset.relative_path));
            ui.small(format!("Expected at {}", asset.resolved_path));
        }
        None => {
            ui.small(format!("{}: no reference yet", label));
        }
    }
}

fn show_inventory_thumbnail(
    ui: &mut egui::Ui,
    bundle: &DatapackBundle,
    template: &crate::data::datapacks::ItemTemplate,
    inventory_item: &crate::game::state::InventoryEntry,
    viewer_request: &mut Option<AssetViewerRequest>,
) {
    let thumbnail = build_inventory_thumbnail_model(bundle, template);

    if thumbnail.uses_engine_fallback {
        let response = ui.add_sized([44.0, 44.0], egui::Button::new("Item"));
        if response.clicked() {
            *viewer_request = Some(build_item_asset_viewer_request(
                bundle,
                inventory_item,
                template,
            ));
        }
        return;
    }

    let Some(image_path) = &thumbnail.image_path else {
        return;
    };
    let image_uri = local_file_uri(image_path);
    let response = ui.add(
        egui::Image::from_uri(image_uri)
            .fit_to_exact_size(egui::vec2(44.0, 44.0))
            .sense(egui::Sense::click()),
    );
    response.clone().on_hover_text(&thumbnail.hover_text);
    if response.clicked() {
        *viewer_request = Some(build_item_asset_viewer_request(
            bundle,
            inventory_item,
            template,
        ));
    }
}

fn show_media_panel_preview(
    ui: &mut egui::Ui,
    media_panel: &MediaPanelState,
    viewer_request: &mut Option<AssetViewerRequest>,
) {
    let preview = build_media_panel_preview_model(media_panel);

    if preview.uses_engine_fallback {
        show_engine_fallback_card(ui);
        return;
    }

    if let Some(image_path) = &preview.image_path {
        let image_uri = local_file_uri(image_path);
        let preview_width = ui.available_width().clamp(180.0, 360.0);
        let preview_max_height = 240.0;

        egui::Frame::new()
            .fill(egui::Color32::from_rgb(18, 23, 20))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(72, 82, 72)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_width(preview_width);

                let response = ui.add(
                    egui::Image::from_uri(image_uri)
                        .fit_to_original_size(1.0)
                        .max_size(egui::vec2(preview_width, preview_max_height))
                        .sense(egui::Sense::click()),
                );
                response
                    .clone()
                    .on_hover_text("Click to open the focused media viewer.");
                if response.clicked() {
                    *viewer_request = Some(build_media_panel_asset_viewer_request(media_panel));
                }

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.small(
                        egui::RichText::new(&preview.title)
                            .strong()
                            .color(egui::Color32::from_rgb(224, 228, 224)),
                    );
                    if let Some(source_label) = &preview.source_label {
                        ui.small(
                            egui::RichText::new(format!("| {}", source_label))
                                .color(egui::Color32::from_rgb(148, 162, 148)),
                        );
                    }
                });
                if let Some(role_line) = &preview.role_line {
                    ui.small(role_line);
                }
                ui.small("Click preview to enlarge");
            });
        return;
    }

    egui::Frame::new()
        .fill(egui::Color32::from_rgb(18, 23, 20))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(72, 82, 72)))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(ui.available_width().max(180.0), 140.0));
            ui.vertical_centered(|ui| {
                ui.add_space(24.0);
                ui.label(egui::RichText::new(&preview.empty_title).strong());
                ui.small(&preview.empty_detail);
            });
        });
}

fn local_file_uri(path: &str) -> String {
    format!("file://{}", path.replace('\\', "/"))
}

fn show_engine_fallback_card(ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.set_min_size(egui::vec2(420.0, 260.0));
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.label(egui::RichText::new("Chatty Quest").strong().size(28.0));
            ui.small("RD Engine fallback viewer state");
            ui.add_space(12.0);
            ui.small("No pack-owned image could be resolved for this focus.");
        });
    });
}

fn render_map_layout(
    ui: &mut egui::Ui,
    layout: &GeneratedMapLayout,
    run: &RunState,
    fog_mode: &str,
    queued_command: &mut Option<String>,
    viewer_request: &mut Option<AssetViewerRequest>,
) {
    show_map_legend(ui);
    ui.add_space(6.0);
    show_current_exit_strip(ui, layout, run, queued_command);
    ui.add_space(6.0);

    let tile_lookup = layout
        .tiles
        .iter()
        .map(|tile| ((tile.grid_x, tile.grid_y), tile))
        .collect::<HashMap<_, _>>();

    egui::Grid::new("map_tile_grid")
        .spacing(egui::vec2(6.0, 6.0))
        .show(ui, |ui| {
            for grid_y in 0..layout.height {
                for grid_x in 0..layout.width {
                    if let Some(tile) = tile_lookup.get(&(grid_x, grid_y)) {
                        render_map_tile(ui, tile, run, fog_mode, queued_command, viewer_request);
                    } else {
                        ui.allocate_ui(egui::vec2(74.0, 92.0), |_ui| {});
                    }
                }
                ui.end_row();
            }
        });
}

fn render_map_tile(
    ui: &mut egui::Ui,
    tile: &MapTileLayout,
    run: &RunState,
    fog_mode: &str,
    queued_command: &mut Option<String>,
    viewer_request: &mut Option<AssetViewerRequest>,
) {
    let display = build_map_tile_display(run, tile, fog_mode);
    let hover = build_map_tile_hover(tile, &display);

    let frame_fill = if matches!(display.visibility, MapTileVisibilityModel::Hidden) {
        egui::Color32::from_rgb(20, 24, 22)
    } else if matches!(display.visibility, MapTileVisibilityModel::Hinted) {
        egui::Color32::from_rgb(22, 30, 24)
    } else if display.is_current {
        egui::Color32::from_rgb(28, 42, 34)
    } else if display.is_visited {
        egui::Color32::from_rgb(24, 31, 26)
    } else {
        egui::Color32::from_rgb(22, 27, 22)
    };
    let frame_stroke = if display.is_current {
        egui::Stroke::new(2.0, egui::Color32::from_rgb(125, 196, 255))
    } else if matches!(display.visibility, MapTileVisibilityModel::Hinted) {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(192, 208, 132))
    } else if display.is_adjacent_exit {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(158, 214, 120))
    } else if display.is_visible {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(86, 102, 86))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(54, 60, 54))
    };

    egui::Frame::new()
        .fill(frame_fill)
        .stroke(frame_stroke)
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(86.0, 124.0));
            draw_tile_connectors(
                ui,
                tile,
                display.is_visible,
                display.is_current,
                display.is_adjacent_exit,
            );

            if display.is_fully_visible {
                let response = if tile.using_engine_fallback {
                    ui.add_sized([64.0, 64.0], egui::Button::new("Fallback"))
                } else {
                    let image_uri = local_file_uri(&tile.thumbnail_path);
                    ui.add(
                        egui::Image::from_uri(image_uri)
                            .fit_to_exact_size(egui::vec2(64.0, 64.0))
                            .sense(egui::Sense::click()),
                    )
                };
                response.clone().on_hover_ui(|ui| {
                    show_map_tile_hover(ui, &hover);
                });

                if response.clicked() {
                    *viewer_request = Some(build_map_location_asset_viewer_request(tile, run));
                }
            } else if matches!(display.visibility, MapTileVisibilityModel::Hinted) {
                let response = render_hinted_tile_face(ui);
                response.clone().on_hover_ui(|ui| {
                    show_map_tile_hover(ui, &hover);
                });
                if response.clicked() {
                    *queued_command = Some(format!("go {}", tile.location_id));
                }
            } else {
                let response = ui.add_sized(
                    [64.0, 64.0],
                    egui::Button::new(egui::RichText::new("?").size(22.0)),
                );
                response.on_hover_text("Unknown location. Discover it through play.");
            }

            ui.small(format!("{} {}", display.marker, display.title));
            if display.is_fully_visible {
                ui.horizontal_wrapped(|ui| {
                    if display.is_current {
                        badge_chip(ui, "You", egui::Color32::from_rgb(125, 196, 255));
                    }
                    if display.show_loot_badge {
                        badge_chip(ui, "Loot", egui::Color32::from_rgb(158, 214, 120));
                    }
                    if display.show_threat_badge {
                        badge_chip(ui, "Threat", egui::Color32::from_rgb(214, 120, 120));
                    }
                    if display.show_objective_badge {
                        badge_chip(ui, "Objective", egui::Color32::from_rgb(241, 202, 104));
                    }
                    if display.show_locked_badge {
                        badge_chip(ui, "Locked", egui::Color32::from_rgb(196, 156, 98));
                    }
                    if display.show_broken_badge {
                        badge_chip(ui, "Broken", egui::Color32::from_rgb(188, 116, 92));
                    }
                    if display.show_barricaded_badge {
                        badge_chip(ui, "Barricaded", egui::Color32::from_rgb(104, 184, 156));
                    }
                });

                if display.show_move_button && ui.small_button("Move").clicked() {
                    *queued_command = Some(format!("go {}", tile.location_id));
                }
            } else if display.show_advance_button {
                ui.horizontal_wrapped(|ui| {
                    badge_chip(ui, "Exit", egui::Color32::from_rgb(192, 208, 132));
                });
                if ui.small_button("Advance").clicked() {
                    *queued_command = Some(format!("go {}", tile.location_id));
                }
            }
        });
}

fn show_current_exit_strip(
    ui: &mut egui::Ui,
    layout: &GeneratedMapLayout,
    run: &RunState,
    queued_command: &mut Option<String>,
) {
    let exits = build_map_exit_buttons(layout, run);

    ui.group(|ui| {
        ui.label("Current exits");
        if exits.is_empty() {
            ui.small("No connected exits surfaced from this location.");
            return;
        }

        ui.horizontal_wrapped(|ui| {
            for exit in exits {
                if ui
                    .small_button(format!(
                        "{} {}{}",
                        exit.direction_label,
                        exit.destination_name,
                        location_state_suffix(exit.locked, exit.broken, exit.barricaded)
                    ))
                    .clicked()
                {
                    *queued_command = Some(format!("go {}", exit.destination_id));
                }
            }
        });
    });
}

fn render_hinted_tile_face(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(64.0, 64.0), egui::Sense::click());
    let fill = if response.hovered() {
        egui::Color32::from_rgb(56, 68, 42)
    } else {
        egui::Color32::from_rgb(42, 52, 34)
    };
    let stroke = if response.hovered() {
        egui::Stroke::new(1.5, egui::Color32::from_rgb(214, 228, 154))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(170, 186, 114))
    };

    ui.painter().rect(
        rect,
        egui::CornerRadius::same(6),
        fill,
        stroke,
        egui::StrokeKind::Outside,
    );
    ui.painter().text(
        rect.center_top() + egui::vec2(0.0, 13.0),
        egui::Align2::CENTER_CENTER,
        "?",
        egui::FontId::proportional(24.0),
        egui::Color32::from_rgb(222, 229, 184),
    );
    ui.painter().text(
        rect.center_bottom() + egui::vec2(0.0, -18.0),
        egui::Align2::CENTER_CENTER,
        "EXIT",
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(205, 214, 150),
    );
    ui.painter().text(
        rect.center_bottom() + egui::vec2(0.0, -6.0),
        egui::Align2::CENTER_CENTER,
        "Scout ahead",
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(156, 166, 125),
    );

    response
}

fn show_map_legend(ui: &mut egui::Ui) {
    let legend = build_map_legend();
    ui.group(|ui| {
        ui.label("Legend");
        ui.horizontal_wrapped(|ui| {
            for badge in &legend.badges {
                let color = match badge.label.as_str() {
                    "You" => egui::Color32::from_rgb(125, 196, 255),
                    "Loot" => egui::Color32::from_rgb(158, 214, 120),
                    "Threat" => egui::Color32::from_rgb(214, 120, 120),
                    "Objective" => egui::Color32::from_rgb(241, 202, 104),
                    "Barricaded" => egui::Color32::from_rgb(104, 184, 156),
                    _ => egui::Color32::from_rgb(148, 148, 148),
                };
                badge_chip(ui, &badge.label, color);
                ui.small(&badge.description);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for row in &legend.marker_rows {
                ui.small(row);
            }
        });
        ui.small(&legend.fog_line);
    });
}

fn show_map_tile_hover(ui: &mut egui::Ui, hover: &MapTileHoverModel) {
    ui.label(egui::RichText::new(&hover.title).strong());
    if let Some(location_id_line) = &hover.location_id_line {
        ui.small(location_id_line);
    }
    ui.small(&hover.status_line);

    if !hover.detail_rows.is_empty() {
        ui.separator();
        for row in &hover.detail_rows {
            ui.small(row);
        }
    }

    ui.separator();
    for row in &hover.footer_rows {
        ui.small(row);
    }
}

fn draw_tile_connectors(
    ui: &mut egui::Ui,
    tile: &MapTileLayout,
    is_known: bool,
    is_current: bool,
    is_adjacent_exit: bool,
) {
    let rect = ui.max_rect();
    let center = rect.center_top() + egui::vec2(0.0, 34.0);
    let connector_color = if !is_known {
        egui::Color32::from_rgb(52, 58, 54)
    } else if is_current {
        egui::Color32::from_rgb(125, 196, 255)
    } else if is_adjacent_exit {
        egui::Color32::from_rgb(158, 214, 120)
    } else {
        egui::Color32::from_rgb(94, 112, 94)
    };
    let stroke = egui::Stroke::new(if is_current { 2.5 } else { 1.5 }, connector_color);
    let painter = ui.painter();

    painter.circle_filled(
        center,
        if is_current { 4.5 } else { 3.0 },
        connector_color.gamma_multiply(if is_known { 0.9 } else { 0.55 }),
    );

    if tile.connects_north {
        painter.line_segment(
            [
                center + egui::vec2(0.0, -22.0),
                center + egui::vec2(0.0, -34.0),
            ],
            stroke,
        );
    }
    if tile.connects_east {
        painter.line_segment(
            [
                center + egui::vec2(22.0, 0.0),
                center + egui::vec2(34.0, 0.0),
            ],
            stroke,
        );
    }
    if tile.connects_south {
        painter.line_segment(
            [
                center + egui::vec2(0.0, 22.0),
                center + egui::vec2(0.0, 34.0),
            ],
            stroke,
        );
    }
    if tile.connects_west {
        painter.line_segment(
            [
                center + egui::vec2(-22.0, 0.0),
                center + egui::vec2(-34.0, 0.0),
            ],
            stroke,
        );
    }
}

fn show_datapack_status(
    ui: &mut egui::Ui,
    datapacks: &DatapackCatalog,
    selected_record: Option<&SelectedDatapackPreview>,
    selected_invalid: Option<&SelectedInvalidDatapackPreview>,
) {
    ui.group(|ui| {
        ui.heading("Cartridge Status");
        ui.label(format!(
            "Ready cartridges discovered: {}",
            datapacks.valid.len()
        ));
        ui.small(format!(
            "Broken cartridges discovered: {}",
            datapacks.invalid.len()
        ));

        if let Some(record) = selected_record {
            ui.colored_label(
                egui::Color32::from_rgb(40, 140, 70),
                "Selected cartridge: ready.",
            );
            if let Some(boundary) = &record.boundary_response {
                ui.small(format!("Boundary response preview: {}", boundary));
            }
            ui.small(format!(
                "Pack presentation: {} narrator briefs | {} media refs | {} sensory templates",
                record.narrator_brief_count,
                record.media_reference_count,
                record.sensory_template_count
            ));
            if let Some(dm_style) = &record.dm_style_preview {
                ui.small(format!("DM capsule preview: {}", dm_style));
            }
            if let Some(world_tone) = &record.world_tone_preview {
                ui.small(format!("World tone preview: {}", world_tone));
            }
        } else if let Some(invalid) = selected_invalid {
            ui.colored_label(
                egui::Color32::from_rgb(180, 40, 40),
                "Selected cartridge: failed validation.",
            );
            let label = invalid
                .display_name
                .as_deref()
                .unwrap_or(invalid.folder_name.as_str());
            ui.small(format!("Label: {}", label));
            for error in &invalid.errors {
                ui.small(format!("- {}", error));
            }
        }

        if datapacks.invalid.is_empty() {
            ui.small("No invalid datapacks detected.");
        } else {
            ui.separator();
            ui.label("Invalid datapacks");
            for invalid in &datapacks.invalid {
                let label = invalid
                    .display_name
                    .as_deref()
                    .unwrap_or(invalid.folder_name.as_str());
                let version = invalid
                    .version
                    .as_deref()
                    .map(|version| format!(" v{}", version))
                    .unwrap_or_default();
                ui.small(format!("{}{} ({})", label, version, invalid.folder_name));
                for error in &invalid.errors {
                    ui.small(format!("- {}", error));
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use crate::collection::{
        PlayerTemplate, PlayerTemplateAdmission, PlayerTemplateKind, PlayerTemplateMechanics,
        PlayerTemplateMediaReferences,
    };
    use crate::game::{ActorDisposition, ActorState};

    use super::{player_template_capability_summary, player_template_library_summary};

    #[test]
    fn library_summary_preserves_typed_mechanics_and_actor_contract() {
        let template = PlayerTemplate {
            id: "fixture_npc".to_owned(),
            kind: PlayerTemplateKind::Npc,
            name: "Fixture Guide".to_owned(),
            description: "A UI fixture.".to_owned(),
            damage: 0,
            condition: None,
            mechanics: Some(PlayerTemplateMechanics::Npc {
                faction: "helpers".to_owned(),
            }),
            tags: vec!["guide".to_owned()],
            actor: Some(ActorState::configured(
                Some(8),
                Some(3),
                true,
                true,
                true,
                true,
            )),
            disposition: ActorDisposition::Friendly,
            source_datapack_id: "fixture".to_owned(),
            enabled: true,
            provenance: "narrator_approved".to_owned(),
            admission: PlayerTemplateAdmission::default(),
            media_references: PlayerTemplateMediaReferences::default(),
        };

        assert!(player_template_library_summary(&template).contains("NPC | faction: helpers"));
        let contract = player_template_capability_summary(&template)
            .expect("actor-capable template has a contract summary");
        assert!(contract.contains("combat"));
        assert!(contract.contains("Friendly"));
    }
}
