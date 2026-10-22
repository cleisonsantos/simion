#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editor;
mod icon;
mod icons;
mod markdown;
mod model;
mod overlays;
mod search;
mod sidebar;
mod store;
mod theme;
mod ui;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Simion")
            .with_app_id("simion")
            .with_inner_size([1240.0, 820.0])
            .with_min_inner_size([720.0, 520.0])
            .with_icon(Arc::new(icon::app_icon())),
        ..Default::default()
    };

    eframe::run_native(
        "Simion",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
