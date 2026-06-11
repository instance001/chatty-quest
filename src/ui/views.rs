use std::collections::HashMap;

use eframe::egui;

use crate::data::datapacks::{DatapackBundle, DatapackCatalog};
use crate::diagnostics::DiagnosticReport;
use crate::game::RunState;
use crate::media::MediaPanelState;
use crate::ui::{
    MapTileHoverModel, MapTileVisibilityModel, build_asset_viewer_chrome, build_character_actions,
    build_character_summary, build_diagnostics_summary, build_game_action_bar,
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
}

pub struct SetupScreenModel<'a> {
    pub selected_datapack: &'a mut String,
    pub difficulty: &'a mut f32,
    pub chaos_mode: &'a mut f32,
    pub fog_mode: &'a mut String,
    pub dm_capsule: &'a mut String,
    pub cpu_helper_model: &'a mut String,
    pub gpu_narrator_model: &'a mut String,
    pub datapacks: &'a DatapackCatalog,
    pub selected_record: Option<SelectedDatapackPreview>,
    pub header_image_path: Option<&'a str>,
}

pub struct SplashScreenModel<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    pub footer: &'a str,
    pub image_path: Option<&'a str>,
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
    pub boundary_response: Option<String>,
    pub dm_style_preview: Option<String>,
    pub world_tone_preview: Option<String>,
}

pub fn show_setup_screen(ctx: &egui::Context, model: SetupScreenModel<'_>) -> SetupScreenAction {
    let mut action = SetupScreenAction::None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(24.0);
        if let Some(image_path) = model.header_image_path {
            let image_uri = local_file_uri(image_path);
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
                        .selected_text(model.selected_datapack.as_str())
                        .show_ui(ui, |ui| {
                            for record in &model.datapacks.valid {
                                ui.selectable_value(
                                    model.selected_datapack,
                                    record.summary.display_name.clone(),
                                    &record.summary.display_name,
                                );
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
                            "Presentation coverage: {} narrator briefs | {} media refs",
                            record.narrator_brief_count, record.media_reference_count
                        ));
                    }

                    ui.add_space(8.0);
                    ui.label("Difficulty");
                    ui.add(
                        egui::Slider::new(model.difficulty, 0.0..=1.0)
                            .show_value(false)
                            .text("Difficulty"),
                    );
                    ui.small("Mechanical pressure dial. Separate from tone.");

                    ui.add_space(8.0);
                    ui.label("Chaos mode");
                    ui.add(
                        egui::Slider::new(model.chaos_mode, 0.0..=1.0)
                            .show_value(false)
                            .text("Chaos"),
                    );
                    ui.small("Narration looseness dial. Never truth ownership.");

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
                    ui.heading("Future Model Lanes");
                    ui.label("Bookkeeper / summary helper");
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(model.cpu_helper_model),
                    );
                    ui.small("Reserved for a future CPU-friendly helper lane.");

                    ui.add_space(8.0);
                    ui.label("Main narrator");
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(model.gpu_narrator_model),
                    );
                    ui.small("Reserved for a future GPU-backed narrator lane.");
                });

                ui.add_space(12.0);

                ui.group(|ui| {
                    ui.heading("Run Controls");

                    let can_generate = model.selected_record.is_some();
                    if ui
                        .add_enabled(can_generate, egui::Button::new("Generate Game"))
                        .clicked()
                    {
                        action = SetupScreenAction::GenerateGame;
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Load Game").clicked() {
                            action = SetupScreenAction::LoadGame;
                        }
                        ui.add_enabled(false, egui::Button::new("Advanced Generation"));
                    });

                    ui.small("Tabs stay hidden until a run exists, keeping setup uncluttered.");
                });
            });
        });

        ui.add_space(16.0);
        show_datapack_status(ui, model.datapacks, model.selected_record.as_ref());
        ui.add_space(10.0);
        ui.label("`v0.1` keeps setup lean on purpose: choose the run shape, then drop straight into the deterministic shell.");
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
                ui.add_space(6.0);
                ui.small("A deterministic chat-forward adventure engine.");
                ui.add_space(18.0);

                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 22, 18))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(68, 81, 66)))
                    .corner_radius(egui::CornerRadius::same(18))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width().min(880.0));
                        if let Some(image_path) = model.image_path {
                            let image_uri = local_file_uri(image_path);
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
    current_input: &mut String,
) -> Option<String> {
    let mut queued_command = None;
    let action_bar = build_game_action_bar(current_run, current_bundle);

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
) -> (Option<String>, Option<AssetViewerRequest>) {
    let mut queued_command = None;
    let mut viewer_request = None;
    let map_layout = current_bundle
        .zip(current_run)
        .map(|(bundle, run)| build_map_layout(bundle, run));

    egui::SidePanel::left("map_panel")
        .resizable(true)
        .default_width(240.0)
        .show(ctx, |ui| {
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
                                    .button(format!("Go to {}", exit.destination_name))
                                    .clicked()
                                {
                                    queued_command = Some(format!("go {}", exit.destination_id));
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
                            ui.small(format!("{} {}", location.marker, location.location_name));
                        }
                    }
                }
            } else {
                ui.label("No active run.");
            }
        });

    egui::SidePanel::right("media_panel")
        .resizable(true)
        .default_width(260.0)
        .show(ctx, |ui| {
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

            if !display.future_hook_rows.is_empty() {
                ui.separator();
                ui.label("Future hook keys");
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

    (queued_command, viewer_request)
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
                let item = run.inventory.iter().find(|item| item.id == row.item_id);
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

                            if row.show_view_button
                                && ui.small_button("View").clicked()
                                && let (Some(template), Some(item)) = (template, item)
                            {
                                viewer_request =
                                    Some(build_item_asset_viewer_request(bundle, item, template));
                            }
                        });
                        ui.small(&row.description_line);
                        if let Some(template_warning) = &row.template_warning {
                            ui.small(template_warning);
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

pub fn show_character_tab(
    ctx: &egui::Context,
    current_run: Option<&RunState>,
    current_bundle: Option<&DatapackBundle>,
) -> Option<AssetViewerRequest> {
    let mut viewer_request = None;

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Character");
        if let Some(run) = current_run {
            let summary = build_character_summary(run, current_bundle);
            let actions = build_character_actions(&summary);
            ui.label("Character stats");
            ui.separator();
            ui.label(format!("Datapack id: {}", summary.datapack_id));
            ui.label(format!("HP: {} / {}", summary.hp, summary.max_hp));
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
            ui.small(format!(
                "Enemies defeated: {} | Bosses defeated: {}",
                summary.enemies_defeated, summary.bosses_defeated
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
                        run.inventory.iter().find(|item| item.id == *equipped_item_id)
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

    viewer_request
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
                ui.small(format!("{}:", invalid.folder_name));
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
                        "{} {}",
                        exit.direction_label, exit.destination_name
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
) {
    ui.group(|ui| {
        ui.heading("Datapack Status");
        ui.label(format!(
            "Valid datapacks discovered: {}",
            datapacks.valid.len()
        ));

        if let Some(record) = selected_record {
            if let Some(boundary) = &record.boundary_response {
                ui.small(format!("Boundary response preview: {}", boundary));
            }
            ui.small(format!(
                "Pack presentation: {} narrator briefs | {} media refs",
                record.narrator_brief_count, record.media_reference_count
            ));
            if let Some(dm_style) = &record.dm_style_preview {
                ui.small(format!("DM capsule preview: {}", dm_style));
            }
            if let Some(world_tone) = &record.world_tone_preview {
                ui.small(format!("World tone preview: {}", world_tone));
            }
        }

        if datapacks.invalid.is_empty() {
            ui.small("No invalid datapacks detected.");
        } else {
            ui.separator();
            ui.label("Invalid datapacks");
            for invalid in &datapacks.invalid {
                ui.small(format!("{}:", invalid.folder_name));
                for error in &invalid.errors {
                    ui.small(format!("- {}", error));
                }
            }
        }
    });
}
