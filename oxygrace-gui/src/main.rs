//! Oxygrace GUI: an interactive viewer/editor for Grace `.agr` plot files,
//! built on the headless oxygrace renderer.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
#[cfg(not(target_arch = "wasm32"))]
mod args;
mod edit;
mod file;
mod icons;
mod inspector;
mod plot_view;
mod render;
mod theme;
mod tree;
mod undo;

#[cfg(not(target_arch = "wasm32"))]
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

/// Web entry point: attach to the `oxygrace_canvas` element of index.html.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("oxygrace_canvas")
            .expect("index.html must have a canvas with id oxygrace_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("oxygrace_canvas is not a canvas element");
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
