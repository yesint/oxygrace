//! Oxygrace GUI: an interactive viewer/editor for Grace `.agr` plot files,
//! built on the headless oxygrace renderer.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod args;
mod edit;
mod file;
mod inspector;
mod plot_view;
mod render;
mod theme;
mod tree;
mod undo;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Oxygrace"),
        ..Default::default()
    };
    eframe::run_native(
        "oxygrace",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
