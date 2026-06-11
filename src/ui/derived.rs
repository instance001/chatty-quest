use std::collections::{HashMap, HashSet, VecDeque};

use crate::data::datapacks::DatapackBundle;
use crate::diagnostics::DiagnosticReport;
use crate::game::RunState;
use crate::media::{MediaPanelState, resolve_location_tile_image, resolve_primary_image};
use crate::ui::views::{AssetViewerRequest, GeneratedMapLayout, MapTileLayout};

#[derive(Clone)]
pub struct StatusChipModel {
    pub label: String,
    pub value: String,
}

#[derive(Clone)]
pub struct InventoryRowModel {
    pub item_id: String,
    pub item_name: String,
    pub description: String,
    pub damage: i32,
    pub equipped: bool,
    pub has_template: bool,
}

#[derive(Clone)]
pub struct CharacterSummaryModel {
    pub datapack_id: String,
    pub hp: i32,
    pub max_hp: i32,
    pub current_location_id: String,
    pub current_location_name: Option<String>,
    pub objective_complete: bool,
    pub active_objective_id: String,
    pub active_objective_description: String,
    pub enemies_defeated: usize,
    pub bosses_defeated: usize,
    pub rolling_summary: Vec<String>,
    pub pack_folder: Option<String>,
    pub objective_tags: Option<String>,
    pub equipped_item_id: Option<String>,
}

#[derive(Clone)]
pub struct CharacterActionModel {
    pub pack_folder_line: Option<String>,
    pub objective_tags_line: Option<String>,
    pub view_current_location_label: Option<String>,
    pub view_equipped_item_label: Option<String>,
}

#[derive(Clone)]
pub struct ExitRowModel {
    pub destination_id: String,
    pub destination_name: String,
}

#[derive(Clone)]
pub struct LocalItemActionRowModel {
    pub item_name: String,
    pub inspect_command: String,
    pub take_command: String,
}

#[derive(Clone)]
pub struct KnownLocationRowModel {
    pub location_name: String,
    pub marker: String,
}

#[derive(Clone)]
pub struct GameSidebarModel {
    pub current_location_name: String,
    pub current_location_description: String,
    pub current_location_tags: Vec<String>,
    pub connected_exits: Vec<ExitRowModel>,
    pub local_item_count: usize,
    pub known_locations: Vec<KnownLocationRowModel>,
}

#[derive(Clone)]
pub struct ActionButtonModel {
    pub label: String,
    pub command: String,
}

#[derive(Clone)]
pub struct GameActionBarModel {
    pub primary_actions: Vec<ActionButtonModel>,
    pub quick_exits: Vec<ExitRowModel>,
    pub command_label: String,
    pub command_hint: String,
    pub submit_label: String,
}

#[derive(Clone)]
pub struct InventoryActionRowModel {
    pub item_id: String,
    pub display_name: String,
    pub description_line: String,
    pub inspect_command: String,
    pub equip_command: Option<String>,
    pub use_command: String,
    pub show_view_button: bool,
    pub template_warning: Option<String>,
}

#[derive(Clone)]
pub struct DiagnosticsSummaryModel {
    pub narrator_attached: String,
    pub save_path: String,
    pub current_save_version: String,
    pub datapack_schema_version: String,
    pub loaded_run: String,
    pub loaded_save_version: String,
    pub valid_datapacks: String,
    pub invalid_datapack_count: String,
    pub active_bundle_name: String,
    pub template_counts: Option<String>,
    pub presentation_coverage: Option<String>,
    pub media_assets: Option<String>,
    pub invalid_datapack_rows: Vec<DiagnosticsInvalidPackRow>,
    pub media_missing_summary: Option<String>,
    pub media_missing_rows: Vec<DiagnosticsMediaMissingRow>,
    pub run_health_rows: Vec<String>,
    pub environment_rows: Vec<String>,
    pub warning_rows: Vec<String>,
    pub event_counters: String,
    pub recent_events: Vec<String>,
}

#[derive(Clone)]
pub struct DiagnosticsInvalidPackRow {
    pub folder_name: String,
    pub errors: Vec<String>,
}

#[derive(Clone)]
pub struct DiagnosticsMediaMissingRow {
    pub summary: String,
    pub expected_path: String,
}

#[derive(Clone)]
pub struct MediaPanelDisplayModel {
    pub role_line: Option<String>,
    pub media_state_line: String,
    pub image_source_line: Option<String>,
    pub placeholder_message: String,
    pub narrator_brief_line: Option<String>,
    pub current_location_name_line: Option<String>,
    pub current_location_description_line: Option<String>,
    pub world_tone_line: Option<String>,
    pub boundary_rule_line: Option<String>,
    pub active_cue_rows: Vec<String>,
    pub future_hook_rows: Vec<String>,
    pub encounter_snapshot_rows: Vec<String>,
}

#[derive(Clone)]
pub struct InventoryThumbnailModel {
    pub image_path: Option<String>,
    pub uses_engine_fallback: bool,
    pub hover_text: String,
}

#[derive(Clone)]
pub struct MediaPanelPreviewModel {
    pub uses_engine_fallback: bool,
    pub image_path: Option<String>,
    pub source_label: Option<String>,
    pub role_line: Option<String>,
    pub title: String,
    pub empty_title: String,
    pub empty_detail: String,
}

#[derive(Clone)]
pub struct AssetViewerChromeModel {
    pub close_label: String,
    pub missing_image_line: String,
    pub datapack_fallback_line: String,
    pub engine_fallback_line: String,
}

#[derive(Clone)]
pub struct OutcomeBannerModel {
    pub label: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct MapLegendBadgeModel {
    pub label: String,
    pub description: String,
}

#[derive(Clone)]
pub struct MapLegendModel {
    pub badges: Vec<MapLegendBadgeModel>,
    pub marker_rows: Vec<String>,
    pub fog_line: String,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MapTileVisibilityModel {
    Visible,
    Hinted,
    Hidden,
}

#[derive(Clone)]
pub struct MapExitButtonModel {
    pub direction_label: String,
    pub destination_id: String,
    pub destination_name: String,
}

#[derive(Clone)]
pub struct MapTileDisplayModel {
    pub visibility: MapTileVisibilityModel,
    pub is_visible: bool,
    pub is_fully_visible: bool,
    pub is_visited: bool,
    pub is_current: bool,
    pub is_adjacent_exit: bool,
    pub marker: String,
    pub title: String,
    pub show_loot_badge: bool,
    pub show_threat_badge: bool,
    pub show_objective_badge: bool,
    pub show_move_button: bool,
    pub show_advance_button: bool,
}

#[derive(Clone)]
pub struct MapTileHoverModel {
    pub title: String,
    pub location_id_line: Option<String>,
    pub status_line: String,
    pub detail_rows: Vec<String>,
    pub footer_rows: Vec<String>,
}

pub fn build_inventory_rows(
    run: &RunState,
    bundle: Option<&DatapackBundle>,
) -> Vec<InventoryRowModel> {
    run.inventory
        .iter()
        .map(|item| InventoryRowModel {
            item_id: item.id.clone(),
            item_name: item.name.clone(),
            description: item.description.clone(),
            damage: item.damage,
            equipped: run.equipped_item_id.as_deref() == Some(item.id.as_str()),
            has_template: bundle
                .is_some_and(|bundle| bundle.items.iter().any(|candidate| candidate.id == item.id)),
        })
        .collect()
}

pub fn build_game_header_chips(run: Option<&RunState>, chaos_mode: f32) -> Vec<StatusChipModel> {
    let mut chips = Vec::new();

    if let Some(run) = run {
        chips.push(StatusChipModel {
            label: "HP".to_owned(),
            value: format!("{} / {}", run.hp, run.max_hp),
        });
        chips.push(StatusChipModel {
            label: "Enemies".to_owned(),
            value: run.enemies_alive.len().to_string(),
        });
        chips.push(StatusChipModel {
            label: "Bosses".to_owned(),
            value: run.bosses_alive.len().to_string(),
        });
        chips.push(StatusChipModel {
            label: "Objective".to_owned(),
            value: run.active_objective.name.clone(),
        });
    }

    chips.push(StatusChipModel {
        label: "Chaos".to_owned(),
        value: format!("{:.0}%", chaos_mode * 100.0),
    });

    chips
}

pub fn build_outcome_banner(run: Option<&RunState>) -> Option<OutcomeBannerModel> {
    let run = run?;

    if run.hp <= 0 {
        Some(OutcomeBannerModel {
            label: "LOSS".to_owned(),
            detail: "The player has been reduced to 0 HP.".to_owned(),
        })
    } else if run.active_objective.completed {
        Some(OutcomeBannerModel {
            label: "WIN".to_owned(),
            detail: "The run objective has been completed.".to_owned(),
        })
    } else {
        None
    }
}

pub fn build_character_summary(
    run: &RunState,
    bundle: Option<&DatapackBundle>,
) -> CharacterSummaryModel {
    let current_location_name = bundle.and_then(|bundle| {
        bundle
            .locations
            .iter()
            .find(|location| location.id == run.current_location_id)
            .map(|location| location.name.clone())
    });

    let objective_tags = bundle.map(|bundle| {
        bundle
            .objectives
            .iter()
            .find(|objective| objective.id == run.active_objective.id)
            .map(|objective| objective.tags.join(", "))
            .unwrap_or_else(|| "none".to_owned())
    });

    CharacterSummaryModel {
        datapack_id: run.datapack_id.clone(),
        hp: run.hp,
        max_hp: run.max_hp,
        current_location_id: run.current_location_id.clone(),
        current_location_name,
        objective_complete: run.active_objective.completed,
        active_objective_id: run.active_objective.id.clone(),
        active_objective_description: run.active_objective.description.clone(),
        enemies_defeated: run.enemies_defeated.len(),
        bosses_defeated: run.bosses_defeated.len(),
        rolling_summary: run.rolling_summary.clone(),
        pack_folder: bundle.map(|bundle| bundle.folder_name.clone()),
        objective_tags,
        equipped_item_id: run.equipped_item_id.clone(),
    }
}

pub fn build_character_actions(summary: &CharacterSummaryModel) -> CharacterActionModel {
    CharacterActionModel {
        pack_folder_line: summary
            .pack_folder
            .as_ref()
            .map(|pack_folder| format!("Pack folder: {}", pack_folder)),
        objective_tags_line: summary
            .objective_tags
            .as_ref()
            .map(|objective_tags| format!("Objective tags: {}", objective_tags)),
        view_current_location_label: Some("View Current Location".to_owned()),
        view_equipped_item_label: summary
            .equipped_item_id
            .as_ref()
            .map(|_| "View Equipped Item".to_owned()),
    }
}

pub fn build_game_sidebar(run: &RunState, bundle: &DatapackBundle) -> Option<GameSidebarModel> {
    let current_location = bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id)?;

    let connected_exits = current_location
        .connections
        .iter()
        .filter_map(|connection_id| {
            bundle
                .locations
                .iter()
                .find(|candidate| candidate.id == *connection_id)
                .map(|destination| ExitRowModel {
                    destination_id: destination.id.clone(),
                    destination_name: destination.name.clone(),
                })
        })
        .collect();

    let local_item_count = run
        .location_items
        .get(&current_location.id)
        .into_iter()
        .flatten()
        .count();

    let known_locations = bundle
        .locations
        .iter()
        .map(|location| {
            let known = run.known_locations.contains(&location.id);
            let visited = run.visited_locations.contains(&location.id);
            let marker = if run.current_location_id == location.id {
                ">"
            } else if visited {
                "*"
            } else if known {
                "-"
            } else {
                " "
            };

            KnownLocationRowModel {
                location_name: location.name.clone(),
                marker: marker.to_owned(),
            }
        })
        .collect();

    Some(GameSidebarModel {
        current_location_name: current_location.name.clone(),
        current_location_description: current_location.description.clone(),
        current_location_tags: current_location.tags.clone(),
        connected_exits,
        local_item_count,
        known_locations,
    })
}

pub fn build_game_action_bar(
    run: Option<&RunState>,
    bundle: Option<&DatapackBundle>,
) -> GameActionBarModel {
    let primary_actions = [
        ("Look", "look"),
        ("Help", "help"),
        ("Attack", "attack"),
        ("Wait", "wait"),
    ]
    .into_iter()
    .map(|(label, command)| ActionButtonModel {
        label: label.to_owned(),
        command: command.to_owned(),
    })
    .collect();

    let quick_exits = run
        .zip(bundle)
        .and_then(|(run, bundle)| build_game_sidebar(run, bundle))
        .map(|sidebar| sidebar.connected_exits)
        .unwrap_or_default();

    GameActionBarModel {
        primary_actions,
        quick_exits,
        command_label: "Command".to_owned(),
        command_hint: "Type an action...".to_owned(),
        submit_label: "Submit".to_owned(),
    }
}

pub fn build_local_item_action_rows(
    run: &RunState,
    bundle: &DatapackBundle,
) -> Vec<LocalItemActionRowModel> {
    let Some(current_location) = bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id)
    else {
        return Vec::new();
    };

    run.location_items
        .get(&current_location.id)
        .into_iter()
        .flatten()
        .filter_map(|item_id| {
            bundle
                .items
                .iter()
                .find(|candidate| candidate.id == *item_id)
                .map(|item| LocalItemActionRowModel {
                    item_name: item.name.clone(),
                    inspect_command: format!("inspect {}", item.id),
                    take_command: format!("take {}", item.id),
                })
        })
        .collect()
}

pub fn build_inventory_action_rows(
    run: &RunState,
    bundle: Option<&DatapackBundle>,
) -> Vec<InventoryActionRowModel> {
    build_inventory_rows(run, bundle)
        .into_iter()
        .map(|row| InventoryActionRowModel {
            item_id: row.item_id.clone(),
            display_name: if row.equipped {
                format!("{} [equipped]", row.item_name)
            } else {
                row.item_name.clone()
            },
            description_line: format!("{} | damage {}", row.description, row.damage),
            inspect_command: format!("inspect {}", row.item_id),
            equip_command: (!row.equipped).then(|| format!("equip {}", row.item_id)),
            use_command: format!("use {}", row.item_id),
            show_view_button: true,
            template_warning: (!row.has_template).then(|| {
                "Warning: template backing for this item could not be resolved.".to_owned()
            }),
        })
        .collect()
}

pub fn build_diagnostics_summary(report: &DiagnosticReport) -> DiagnosticsSummaryModel {
    let invalid_datapacks = report
        .invalid_datapacks_detail
        .iter()
        .map(|invalid| DiagnosticsInvalidPackRow {
            folder_name: invalid.folder_name.clone(),
            errors: invalid.errors.clone(),
        })
        .collect();

    let (media_missing_summary, media_missing_rows) =
        if let Some(media_report) = &report.media_asset_report {
            (
                Some(if media_report.missing_entries.is_empty() {
                    "All referenced media files for the active datapack are present.".to_owned()
                } else {
                    format!(
                        "{} referenced media file(s) are still missing.",
                        media_report.missing_assets
                    )
                }),
                media_report
                    .missing_entries
                    .iter()
                    .map(|entry| DiagnosticsMediaMissingRow {
                        summary: format!(
                            "- {} '{}' {} -> {}",
                            entry.template_kind,
                            entry.template_id,
                            entry.field_name,
                            entry.relative_path
                        ),
                        expected_path: format!("  expected at {}", entry.resolved_path),
                    })
                    .collect(),
            )
        } else {
            (None, Vec::new())
        };

    let run_health_rows = report.run_health.as_ref().map_or_else(Vec::new, |run| {
        vec![
            format!("Location: {}", run.location_id),
            format!("HP: {} / {}", run.hp, run.max_hp),
            format!(
                "Objective complete: {}",
                if run.objective_complete { "yes" } else { "no" }
            ),
            format!("Inventory items: {}", run.inventory_items),
            format!("Known locations: {}", run.known_locations),
            format!(
                "Live threats: {} enemies | {} bosses",
                run.live_enemies, run.live_bosses
            ),
        ]
    });

    let environment_rows = report
        .environment_checks
        .iter()
        .map(|check| {
            format!(
                "{}: {} ({})",
                check.label,
                if check.present { "present" } else { "missing" },
                check.path
            )
        })
        .collect();

    let warning_rows = if report.warnings.is_empty() {
        vec!["No active diagnostic warnings.".to_owned()]
    } else {
        report
            .warnings
            .iter()
            .map(|warning| {
                format!(
                    "- [{}:{}] {}",
                    warning.severity.label(),
                    warning.area.label(),
                    warning.message
                )
            })
            .collect()
    };

    DiagnosticsSummaryModel {
        narrator_attached: if report.narrator_attached {
            "yes"
        } else {
            "no"
        }
        .to_owned(),
        save_path: report.save_path.clone(),
        current_save_version: report.current_save_version.to_string(),
        datapack_schema_version: report.datapack_schema_version.clone(),
        loaded_run: if report.loaded_run { "yes" } else { "no" }.to_owned(),
        loaded_save_version: report
            .loaded_save_version
            .map(|version| version.to_string())
            .unwrap_or_else(|| "none".to_owned()),
        valid_datapacks: report.valid_datapacks.to_string(),
        invalid_datapack_count: report.invalid_datapacks.to_string(),
        active_bundle_name: report
            .active_bundle_name
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
        template_counts: report.template_counts.as_ref().map(|counts| {
            format!(
                "Templates: {} locations | {} items | {} enemies | {} bosses | {} objectives",
                counts.locations, counts.items, counts.enemies, counts.bosses, counts.objectives
            )
        }),
        presentation_coverage: report.presentation_coverage.as_ref().map(|coverage| {
            format!(
                "Presentation: {} narrator briefs | {} media refs",
                coverage.narrator_briefs, coverage.media_references
            )
        }),
        media_assets: report.media_asset_report.as_ref().map(|media_report| {
            format!(
                "Media assets: {} present | {} missing",
                media_report.present_assets, media_report.missing_assets
            )
        }),
        invalid_datapack_rows: invalid_datapacks,
        media_missing_summary,
        media_missing_rows,
        run_health_rows,
        environment_rows,
        warning_rows,
        event_counters: format!(
            "Counters: moves {} | attacks {} | items taken {} | items used {} | waits {} | wins {} | losses {} | rejected {}",
            report.event_counters.moves,
            report.event_counters.attacks,
            report.event_counters.items_taken,
            report.event_counters.items_used,
            report.event_counters.waits,
            report.event_counters.wins,
            report.event_counters.losses,
            report.event_counters.rejections
        ),
        recent_events: report.recent_events.clone(),
    }
}

pub fn build_media_panel_display(media_panel: &MediaPanelState) -> MediaPanelDisplayModel {
    MediaPanelDisplayModel {
        role_line: media_panel
            .selected_display_role
            .as_ref()
            .map(|role| format!("Display role: {}", role)),
        media_state_line: if media_panel.has_missing_media {
            "Media state: waiting on missing referenced files.".to_owned()
        } else {
            "Media state: stable.".to_owned()
        },
        image_source_line: if media_panel.used_engine_fallback {
            Some("Image source: engine fallback".to_owned())
        } else if media_panel.used_datapack_fallback {
            Some("Image source: datapack fallback".to_owned())
        } else {
            None
        },
        placeholder_message: media_panel.placeholder_message.clone(),
        narrator_brief_line: media_panel
            .narrator_brief
            .as_ref()
            .map(|brief| format!("Narrator brief: {}", brief)),
        current_location_name_line: media_panel
            .current_location_name
            .as_ref()
            .map(|name| format!("Current focus location: {}", name)),
        current_location_description_line: media_panel.current_location_description.clone(),
        world_tone_line: media_panel
            .world_tone
            .as_ref()
            .map(|tone| format!("World tone: {}", tone)),
        boundary_rule_line: media_panel
            .boundary_rule
            .as_ref()
            .map(|boundary| format!("Boundary rule: {}", boundary)),
        active_cue_rows: media_panel
            .active_cues
            .iter()
            .take(4)
            .map(|cue| format!("{}: {}", cue.label, cue.detail))
            .collect(),
        future_hook_rows: media_panel
            .future_hook_keys
            .iter()
            .take(6)
            .cloned()
            .collect(),
        encounter_snapshot_rows: media_panel.encounter_snapshot.clone(),
    }
}

pub fn build_inventory_thumbnail_model(
    bundle: &DatapackBundle,
    template: &crate::data::datapacks::ItemTemplate,
) -> InventoryThumbnailModel {
    let resolution = resolve_primary_image(bundle, &template.media);

    InventoryThumbnailModel {
        image_path: (!resolution.using_engine_fallback)
            .then(|| resolution.image_asset.resolved_path.clone()),
        uses_engine_fallback: resolution.using_engine_fallback,
        hover_text: "Click thumbnail to view item art.".to_owned(),
    }
}

pub fn build_media_panel_preview_model(media_panel: &MediaPanelState) -> MediaPanelPreviewModel {
    let image_path = media_panel
        .selected_image
        .as_ref()
        .filter(|asset| asset.present)
        .map(|asset| asset.resolved_path.clone());

    MediaPanelPreviewModel {
        uses_engine_fallback: media_panel.used_engine_fallback,
        image_path,
        source_label: Some(build_media_panel_source_label(media_panel)),
        role_line: media_panel
            .selected_display_role
            .as_ref()
            .map(|role| format!("Role: {}", role)),
        title: media_panel.title.clone(),
        empty_title: "No preview image".to_owned(),
        empty_detail: "The current focus has no resolved image yet.".to_owned(),
    }
}

pub fn build_asset_viewer_chrome() -> AssetViewerChromeModel {
    AssetViewerChromeModel {
        close_label: "Close".to_owned(),
        missing_image_line: "No image path was available for this viewer request.".to_owned(),
        datapack_fallback_line: "Fallback state: datapack placeholder in use.".to_owned(),
        engine_fallback_line: "Fallback state: engine placeholder in use.".to_owned(),
    }
}

pub fn build_map_legend() -> MapLegendModel {
    MapLegendModel {
        badges: vec![
            MapLegendBadgeModel {
                label: "You".to_owned(),
                description: "current location".to_owned(),
            },
            MapLegendBadgeModel {
                label: "Loot".to_owned(),
                description: "items present".to_owned(),
            },
            MapLegendBadgeModel {
                label: "Threat".to_owned(),
                description: "live enemies or boss".to_owned(),
            },
            MapLegendBadgeModel {
                label: "Objective".to_owned(),
                description: "objective target here".to_owned(),
            },
        ],
        marker_rows: vec![
            "> current".to_owned(),
            "* visited".to_owned(),
            "- known".to_owned(),
            "~ hinted exit".to_owned(),
            "? unknown".to_owned(),
            "Green border = reachable exit".to_owned(),
            "Click image to inspect".to_owned(),
        ],
        fog_line: "Fog modes: Full shows all tiles, Known shows discovered tiles plus adjacent exit hints, Visited shows visited tiles plus adjacent exit hints.".to_owned(),
    }
}

pub fn build_map_layout(bundle: &DatapackBundle, run: &RunState) -> GeneratedMapLayout {
    let start_id = bundle
        .locations
        .iter()
        .find(|location| location.id == run.current_location_id)
        .map(|location| location.id.as_str())
        .or_else(|| {
            bundle
                .locations
                .first()
                .map(|location| location.id.as_str())
        })
        .unwrap_or("");

    let mut coordinates = HashMap::<String, (i32, i32)>::new();
    let mut occupied = HashSet::<(i32, i32)>::new();
    let mut queue = VecDeque::<String>::new();

    if !start_id.is_empty() {
        coordinates.insert(start_id.to_owned(), (0, 0));
        occupied.insert((0, 0));
        queue.push_back(start_id.to_owned());
    }

    while let Some(location_id) = queue.pop_front() {
        let Some(location) = bundle
            .locations
            .iter()
            .find(|location| location.id == location_id)
        else {
            continue;
        };
        let Some((base_x, base_y)) = coordinates.get(&location.id).copied() else {
            continue;
        };

        for (index, connection_id) in location.connections.iter().enumerate() {
            if coordinates.contains_key(connection_id) {
                continue;
            }

            if let Some((grid_x, grid_y)) = choose_location_position(
                bundle,
                connection_id,
                &coordinates,
                &occupied,
                base_x,
                base_y,
                index,
            ) {
                coordinates.insert(connection_id.clone(), (grid_x, grid_y));
                occupied.insert((grid_x, grid_y));
                queue.push_back(connection_id.clone());
            }
        }
    }

    let mut fallback_x = occupied.iter().map(|(x, _)| *x).max().unwrap_or(0) + 2;
    for location in &bundle.locations {
        if coordinates.contains_key(&location.id) {
            continue;
        }
        coordinates.insert(location.id.clone(), (fallback_x, 0));
        occupied.insert((fallback_x, 0));
        fallback_x += 1;
    }

    let min_x = coordinates.values().map(|(x, _)| *x).min().unwrap_or(0);
    let min_y = coordinates.values().map(|(_, y)| *y).min().unwrap_or(0);
    let max_x = coordinates.values().map(|(x, _)| *x).max().unwrap_or(0);
    let max_y = coordinates.values().map(|(_, y)| *y).max().unwrap_or(0);
    let normalized_coordinates = coordinates
        .iter()
        .map(|(location_id, (x, y))| {
            (
                location_id.clone(),
                ((x - min_x) as usize, (y - min_y) as usize),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut tiles = bundle
        .locations
        .iter()
        .filter_map(|location| {
            let (grid_x, grid_y) = normalized_coordinates.get(&location.id).copied()?;
            let thumbnail = resolve_location_tile_image(bundle, location);
            Some(MapTileLayout {
                location_id: location.id.clone(),
                name: location.name.clone(),
                grid_x,
                grid_y,
                thumbnail_path: thumbnail.image_asset.resolved_path,
                using_datapack_fallback: thumbnail.using_datapack_fallback,
                using_engine_fallback: thumbnail.using_engine_fallback,
                has_items: run
                    .location_items
                    .get(&location.id)
                    .is_some_and(|items| !items.is_empty()),
                has_live_threats: run
                    .location_enemies
                    .get(&location.id)
                    .is_some_and(|enemies| {
                        enemies
                            .iter()
                            .any(|enemy_id| run.enemies_alive.contains(enemy_id))
                    })
                    || run.location_bosses.get(&location.id).is_some_and(|bosses| {
                        bosses
                            .iter()
                            .any(|boss_id| run.bosses_alive.contains(boss_id))
                    }),
                has_objective_target: location
                    .bosses
                    .iter()
                    .any(|boss_id| boss_id == &run.active_objective.target_boss_id),
                is_connected_to_current: bundle
                    .locations
                    .iter()
                    .find(|candidate| candidate.id == run.current_location_id)
                    .is_some_and(|current_location| {
                        current_location
                            .connections
                            .iter()
                            .any(|connection| connection == &location.id)
                    }),
                connects_north: has_neighbor_connection(
                    location,
                    &normalized_coordinates,
                    grid_x,
                    grid_y,
                    0,
                    -1,
                ),
                connects_east: has_neighbor_connection(
                    location,
                    &normalized_coordinates,
                    grid_x,
                    grid_y,
                    1,
                    0,
                ),
                connects_south: has_neighbor_connection(
                    location,
                    &normalized_coordinates,
                    grid_x,
                    grid_y,
                    0,
                    1,
                ),
                connects_west: has_neighbor_connection(
                    location,
                    &normalized_coordinates,
                    grid_x,
                    grid_y,
                    -1,
                    0,
                ),
            })
        })
        .collect::<Vec<_>>();

    tiles.sort_by_key(|tile| (tile.grid_y, tile.grid_x, tile.location_id.clone()));

    GeneratedMapLayout {
        width: (max_x - min_x + 1).max(1) as usize,
        height: (max_y - min_y + 1).max(1) as usize,
        tiles,
    }
}

pub fn build_media_panel_source_label(media_panel: &MediaPanelState) -> String {
    if media_panel.used_engine_fallback {
        "engine placeholder".to_owned()
    } else if media_panel.used_datapack_fallback {
        "datapack placeholder".to_owned()
    } else if media_panel.selected_display_role.as_deref() == Some("location") {
        "location focus".to_owned()
    } else {
        media_panel
            .selected_display_role
            .as_ref()
            .map(|role| format!("focused {}", role))
            .unwrap_or_else(|| "resolved media focus".to_owned())
    }
}

pub fn build_media_panel_asset_viewer_request(media_panel: &MediaPanelState) -> AssetViewerRequest {
    AssetViewerRequest {
        viewer_id: "media_focus".to_owned(),
        source_kind: "media_focus".to_owned(),
        title: media_panel.title.clone(),
        subtitle: Some(media_panel.subtitle.clone()),
        description: media_panel
            .narrator_brief
            .clone()
            .or_else(|| Some(media_panel.placeholder_message.clone())),
        image_path: media_panel
            .selected_image
            .as_ref()
            .map(|asset| asset.resolved_path.clone()),
        resolved_source_label: Some(build_media_panel_source_label(media_panel)),
        using_datapack_fallback: media_panel.used_datapack_fallback,
        using_engine_fallback: media_panel.used_engine_fallback,
    }
}

pub fn build_map_location_asset_viewer_request(
    tile: &MapTileLayout,
    run: &RunState,
) -> AssetViewerRequest {
    let resolved_source_label = if tile.using_engine_fallback {
        "engine placeholder".to_owned()
    } else if tile.using_datapack_fallback {
        "datapack placeholder".to_owned()
    } else {
        "location tile".to_owned()
    };

    AssetViewerRequest {
        viewer_id: format!("map_tile:{}", tile.location_id),
        source_kind: "location".to_owned(),
        title: tile.name.clone(),
        subtitle: Some(format!("Map tile for {}", run.datapack_display_name)),
        description: Some(format!(
            "Location id: {}. Clicked from the map panel.",
            tile.location_id
        )),
        image_path: Some(tile.thumbnail_path.clone()),
        resolved_source_label: Some(resolved_source_label),
        using_datapack_fallback: tile.using_datapack_fallback,
        using_engine_fallback: tile.using_engine_fallback,
    }
}

pub fn build_location_asset_viewer_request(
    bundle: &DatapackBundle,
    run: &RunState,
    location: &crate::data::datapacks::LocationTemplate,
) -> AssetViewerRequest {
    let resolution = resolve_location_tile_image(bundle, location);
    let resolved_source_label = if resolution.using_engine_fallback {
        "engine placeholder".to_owned()
    } else if resolution.using_datapack_fallback {
        "datapack placeholder".to_owned()
    } else {
        "location media".to_owned()
    };

    AssetViewerRequest {
        viewer_id: format!("character_location:{}", location.id),
        source_kind: "location".to_owned(),
        title: location.name.clone(),
        subtitle: Some(format!("Current location in {}", run.datapack_display_name)),
        description: Some(location.description.clone()),
        image_path: Some(resolution.image_asset.resolved_path),
        resolved_source_label: Some(resolved_source_label),
        using_datapack_fallback: resolution.using_datapack_fallback,
        using_engine_fallback: resolution.using_engine_fallback,
    }
}

pub fn build_item_asset_viewer_request(
    bundle: &DatapackBundle,
    inventory_item: &crate::game::state::InventoryEntry,
    template: &crate::data::datapacks::ItemTemplate,
) -> AssetViewerRequest {
    let resolution = resolve_primary_image(bundle, &template.media);
    let resolved_source_label = if resolution.using_engine_fallback {
        "engine placeholder".to_owned()
    } else if resolution.using_datapack_fallback {
        "datapack placeholder".to_owned()
    } else {
        "item media".to_owned()
    };

    AssetViewerRequest {
        viewer_id: format!("inventory_item:{}", inventory_item.id),
        source_kind: "item".to_owned(),
        title: inventory_item.name.clone(),
        subtitle: Some(format!("Inventory item | damage {}", inventory_item.damage)),
        description: Some(inventory_item.description.clone()),
        image_path: Some(resolution.image_asset.resolved_path),
        resolved_source_label: Some(resolved_source_label),
        using_datapack_fallback: resolution.using_datapack_fallback,
        using_engine_fallback: resolution.using_engine_fallback,
    }
}

pub fn build_map_exit_buttons(
    layout: &GeneratedMapLayout,
    run: &RunState,
) -> Vec<MapExitButtonModel> {
    let Some(current_tile) = layout
        .tiles
        .iter()
        .find(|tile| tile.location_id == run.current_location_id)
    else {
        return Vec::new();
    };

    let mut exits = layout
        .tiles
        .iter()
        .filter(|tile| tile.is_connected_to_current)
        .map(|tile| MapExitButtonModel {
            direction_label: exit_direction_label(current_tile, tile).to_owned(),
            destination_id: tile.location_id.clone(),
            destination_name: tile.name.clone(),
        })
        .collect::<Vec<_>>();

    exits.sort_by(|left, right| {
        left.direction_label
            .cmp(&right.direction_label)
            .then_with(|| left.destination_name.cmp(&right.destination_name))
    });

    exits
}

pub fn build_map_tile_display(
    run: &RunState,
    tile: &MapTileLayout,
    fog_mode: &str,
) -> MapTileDisplayModel {
    let visibility = tile_visibility_under_fog(run, tile, fog_mode);
    let is_visible = matches!(
        visibility,
        MapTileVisibilityModel::Visible | MapTileVisibilityModel::Hinted
    );
    let is_fully_visible = matches!(visibility, MapTileVisibilityModel::Visible);
    let is_visited = run.visited_locations.contains(&tile.location_id);
    let is_current = run.current_location_id == tile.location_id;
    let is_adjacent_exit = is_visible && !is_current && tile.is_connected_to_current;

    let marker = if is_current {
        ">"
    } else if is_visited {
        "*"
    } else if is_fully_visible {
        "-"
    } else if matches!(visibility, MapTileVisibilityModel::Hinted) {
        "~"
    } else {
        "?"
    };

    let title = if is_fully_visible {
        tile.name.clone()
    } else if matches!(visibility, MapTileVisibilityModel::Hinted) {
        "Unknown Exit".to_owned()
    } else {
        "Unknown".to_owned()
    };

    MapTileDisplayModel {
        visibility,
        is_visible,
        is_fully_visible,
        is_visited,
        is_current,
        is_adjacent_exit,
        marker: marker.to_owned(),
        title,
        show_loot_badge: is_fully_visible && tile.has_items,
        show_threat_badge: is_fully_visible && tile.has_live_threats,
        show_objective_badge: is_fully_visible && tile.has_objective_target,
        show_move_button: is_adjacent_exit,
        show_advance_button: matches!(visibility, MapTileVisibilityModel::Hinted),
    }
}

pub fn build_map_tile_hover(
    tile: &MapTileLayout,
    display: &MapTileDisplayModel,
) -> MapTileHoverModel {
    let status_line = if display.is_current {
        "Status: current location"
    } else if matches!(display.visibility, MapTileVisibilityModel::Hinted) {
        "Status: hinted adjacent exit"
    } else if display.is_adjacent_exit {
        "Status: reachable exit from the current location"
    } else if display.is_visited {
        "Status: visited"
    } else {
        "Status: known"
    };

    let mut detail_rows = Vec::new();
    if !matches!(display.visibility, MapTileVisibilityModel::Hinted) {
        if tile.has_items {
            detail_rows.push("Loot present".to_owned());
        }
        if tile.has_live_threats {
            detail_rows.push("Live threat present".to_owned());
        }
        if tile.has_objective_target {
            detail_rows.push("Objective target present".to_owned());
        }
    }

    let footer_rows = if matches!(display.visibility, MapTileVisibilityModel::Hinted) {
        vec![
            "This tile is not fully revealed yet.".to_owned(),
            "You can move here because it touches the current location.".to_owned(),
        ]
    } else if tile.using_engine_fallback {
        vec!["Image source: engine fallback".to_owned()]
    } else if tile.using_datapack_fallback {
        vec!["Image source: datapack fallback".to_owned()]
    } else {
        vec!["Image source: location media".to_owned()]
    };

    MapTileHoverModel {
        title: display.title.clone(),
        location_id_line: (!matches!(display.visibility, MapTileVisibilityModel::Hinted))
            .then(|| format!("Location id: {}", tile.location_id)),
        status_line: status_line.to_owned(),
        detail_rows,
        footer_rows,
    }
}

fn choose_location_position(
    bundle: &DatapackBundle,
    target_location_id: &str,
    coordinates: &HashMap<String, (i32, i32)>,
    occupied: &HashSet<(i32, i32)>,
    base_x: i32,
    base_y: i32,
    seed: usize,
) -> Option<(i32, i32)> {
    let Some(target_location) = bundle
        .locations
        .iter()
        .find(|location| location.id == target_location_id)
    else {
        return None;
    };

    let mut candidates = Vec::new();

    for connected_id in &target_location.connections {
        if let Some((anchor_x, anchor_y)) = coordinates.get(connected_id).copied() {
            for (dx, dy) in direction_order(seed) {
                let candidate = (anchor_x + dx, anchor_y + dy);
                if occupied.contains(&candidate) {
                    continue;
                }
                candidates.push(candidate);
            }
        }
    }

    if candidates.is_empty() {
        candidates.extend(
            direction_order(seed)
                .into_iter()
                .map(|(dx, dy)| (base_x + dx, base_y + dy))
                .filter(|candidate| !occupied.contains(candidate)),
        );
    }

    candidates.sort_unstable();
    candidates.dedup();

    candidates.into_iter().max_by_key(|candidate| {
        score_location_position(target_location, coordinates, occupied, *candidate)
    })
}

fn score_location_position(
    target_location: &crate::data::datapacks::LocationTemplate,
    coordinates: &HashMap<String, (i32, i32)>,
    occupied: &HashSet<(i32, i32)>,
    candidate: (i32, i32),
) -> (i32, i32, i32, i32) {
    let mut connected_neighbors = 0;
    let mut nonconnected_neighbors = 0;
    let mut total_distance = 0;

    for connection_id in &target_location.connections {
        if let Some((x, y)) = coordinates.get(connection_id) {
            let manhattan = (candidate.0 - x).abs() + (candidate.1 - y).abs();
            total_distance += manhattan;
            if manhattan == 1 {
                connected_neighbors += 1;
            }
        }
    }

    for neighbor in orthogonal_neighbors(candidate) {
        if occupied.contains(&neighbor) {
            let is_connected = coordinates.iter().any(|(location_id, coordinate)| {
                *coordinate == neighbor
                    && target_location
                        .connections
                        .iter()
                        .any(|id| id == location_id)
            });
            if !is_connected {
                nonconnected_neighbors += 1;
            }
        }
    }

    let compactness = -(candidate.0.abs() + candidate.1.abs());
    let distance_score = -total_distance;

    (
        connected_neighbors,
        -nonconnected_neighbors,
        distance_score,
        compactness,
    )
}

fn direction_order(seed: usize) -> [(i32, i32); 4] {
    const ORDERS: [[(i32, i32); 4]; 4] = [
        [(1, 0), (0, 1), (-1, 0), (0, -1)],
        [(0, 1), (1, 0), (0, -1), (-1, 0)],
        [(-1, 0), (0, -1), (1, 0), (0, 1)],
        [(0, -1), (-1, 0), (0, 1), (1, 0)],
    ];
    ORDERS[seed % ORDERS.len()]
}

fn orthogonal_neighbors(origin: (i32, i32)) -> [(i32, i32); 4] {
    [
        (origin.0, origin.1 - 1),
        (origin.0 + 1, origin.1),
        (origin.0, origin.1 + 1),
        (origin.0 - 1, origin.1),
    ]
}

fn has_neighbor_connection(
    location: &crate::data::datapacks::LocationTemplate,
    normalized_coordinates: &HashMap<String, (usize, usize)>,
    grid_x: usize,
    grid_y: usize,
    delta_x: isize,
    delta_y: isize,
) -> bool {
    let Some(target_x) = grid_x.checked_add_signed(delta_x) else {
        return false;
    };
    let Some(target_y) = grid_y.checked_add_signed(delta_y) else {
        return false;
    };

    location.connections.iter().any(|connection_id| {
        normalized_coordinates
            .get(connection_id)
            .is_some_and(|(neighbor_x, neighbor_y)| {
                *neighbor_x == target_x && *neighbor_y == target_y
            })
    })
}

fn exit_direction_label(current_tile: &MapTileLayout, target_tile: &MapTileLayout) -> &'static str {
    if target_tile.grid_y + 1 == current_tile.grid_y && target_tile.grid_x == current_tile.grid_x {
        "North"
    } else if target_tile.grid_x == current_tile.grid_x + 1
        && target_tile.grid_y == current_tile.grid_y
    {
        "East"
    } else if target_tile.grid_y == current_tile.grid_y + 1
        && target_tile.grid_x == current_tile.grid_x
    {
        "South"
    } else if target_tile.grid_x + 1 == current_tile.grid_x
        && target_tile.grid_y == current_tile.grid_y
    {
        "West"
    } else {
        "Exit"
    }
}

fn tile_visibility_under_fog(
    run: &RunState,
    tile: &MapTileLayout,
    fog_mode: &str,
) -> MapTileVisibilityModel {
    let is_current = run.current_location_id == tile.location_id;
    let is_known = run.known_locations.contains(&tile.location_id);
    let is_visited = run.visited_locations.contains(&tile.location_id);
    let is_adjacent_exit = tile.is_connected_to_current && !is_current;

    match fog_mode {
        "Full" => MapTileVisibilityModel::Visible,
        "Visited" => {
            if is_current || is_visited {
                MapTileVisibilityModel::Visible
            } else if is_adjacent_exit {
                MapTileVisibilityModel::Hinted
            } else {
                MapTileVisibilityModel::Hidden
            }
        }
        _ => {
            if is_current || is_known {
                MapTileVisibilityModel::Visible
            } else if is_adjacent_exit {
                MapTileVisibilityModel::Hinted
            } else {
                MapTileVisibilityModel::Hidden
            }
        }
    }
}
