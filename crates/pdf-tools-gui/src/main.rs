#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

mod app;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("PDF Tools"),
        ..Default::default()
    };

    eframe::run_native(
        "PDF Tools",
        options,
        Box::new(|cc| Ok(Box::new(app::PdfToolsApp::new(cc)))),
    )
}

// WASM entry point
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
    console_error_panic_hook::set_once();

    let web_options = eframe::WebOptions::default();
    eframe::WebRunner::new()
        .start(
            "pdf_tools_canvas",
            web_options,
            Box::new(|cc| Ok(Box::new(app::PdfToolsApp::new(cc)))),
        )
        .await
        .expect("Failed to start eframe");
}
