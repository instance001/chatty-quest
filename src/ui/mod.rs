//! UI-facing modules for the tabbed shell and gameplay views.

mod derived;
mod views;

pub(crate) use derived::build_character_summary;
pub(crate) use derived::{
    MapTileHoverModel, MapTileVisibilityModel, build_asset_viewer_chrome, build_character_actions,
    build_diagnostics_summary, build_game_action_bar, build_game_header_chips, build_game_sidebar,
    build_inventory_action_rows, build_inventory_thumbnail_model, build_item_asset_viewer_request,
    build_local_item_action_rows, build_location_asset_viewer_request, build_map_exit_buttons,
    build_map_layout, build_map_legend, build_map_location_asset_viewer_request,
    build_map_tile_display, build_map_tile_hover, build_media_panel_asset_viewer_request,
    build_media_panel_display, build_media_panel_preview_model, build_outcome_banner,
};
pub use views::{
    AssetViewerRequest, SelectedDatapackPreview, SetupScreenAction, SetupScreenModel,
    SplashScreenModel, show_asset_viewer, show_character_tab, show_diagnostics_tab,
    show_game_action_bar, show_game_tab, show_inventory_tab, show_setup_screen, show_splash_screen,
};
