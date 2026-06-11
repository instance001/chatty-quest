mod app;
mod data;
mod diagnostics;
mod game;
mod media;
mod runtime;
mod session;
mod ui;

use std::path::Path;

use app::ChattyQuestApp;

const APP_WINDOW_TITLE: &str = "Chatty Quest - RD Engine";
const APP_WINDOW_ICON_PATH: &str = "assets/ui/branding/chatty-quest-token.png";

fn main() -> eframe::Result<()> {
    let viewport = if let Some(icon) = load_window_icon(APP_WINDOW_ICON_PATH) {
        egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title(APP_WINDOW_TITLE)
            .with_icon(icon)
    } else {
        egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 640.0])
            .with_title(APP_WINDOW_TITLE)
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        APP_WINDOW_TITLE,
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(ChattyQuestApp::new(cc)))
        }),
    )
}

fn load_window_icon(path: &str) -> Option<egui::IconData> {
    let image_path = Path::new(path);
    let image = image::open(image_path).ok()?.into_rgba8();
    let (width, height) = image.dimensions();

    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
