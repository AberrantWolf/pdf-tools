#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

mod app;
mod handlers;
mod logger;
mod ui_components;
mod viewer;
mod views;
mod worker;

fn setup_fonts(ctx: &egui::Context) {
    use egui::FontData;
    use egui::epaint::text::{FontInsert, InsertFontFamily};

    // Add Noto Sans as the primary proportional font
    ctx.add_font(FontInsert::new(
        "noto_sans",
        FontData::from_static(include_bytes!("../fonts/NotoSans-Regular.ttf")),
        vec![InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: egui::epaint::text::FontPriority::Highest,
        }],
    ));

    // Add Noto Sans Symbols2 as a fallback for symbols
    ctx.add_font(FontInsert::new(
        "noto_symbols",
        FontData::from_static(include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf")),
        vec![InsertFontFamily {
            family: egui::FontFamily::Proportional,
            priority: egui::epaint::text::FontPriority::Lowest,
        }],
    ));
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Initialize tokio runtime for desktop
    let rt = tokio::runtime::Runtime::new().unwrap();
    let handle = rt.handle().clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("PDF Tools"),
        ..Default::default()
    };

    eframe::run_native(
        "PDF Tools",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(app::PdfToolsApp::new(cc, handle)))
        }),
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
            Box::new(|cc| {
                setup_fonts(&cc.egui_ctx);
                Ok(Box::new(app::PdfToolsApp::new(cc)))
            }),
        )
        .await
        .expect("Failed to start eframe");
}
