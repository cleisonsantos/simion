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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("Simion {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Simion {} — um caderno de blocos local.\n\n\
             Uso:\n  simion              abre o caderno\n  simion --version    mostra a versão\n  simion --help       mostra esta ajuda\n\n\
             Dados: {}\n\
             Projeto: https://github.com/cleisonsantos/simion",
            env!("CARGO_PKG_VERSION"),
            store::data_dir().join("simion.db").display()
        );
        return Ok(());
    }

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
