#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

mod app;
mod handlers;
mod logger;
#[cfg(not(target_arch = "wasm32"))]
mod project;
mod startup;
mod theme;
mod ui_components;
mod viewer;
mod views;
mod worker;

fn setup_fonts(ctx: &egui::Context) {
    use egui::FontData;
    use egui::FontFamily::{Monospace, Name, Proportional};
    use egui::epaint::text::FontPriority::{Highest, Lowest};
    use egui::epaint::text::{FontInsert, InsertFontFamily};

    let fam = |family, priority| InsertFontFamily { family, priority };

    // Noto Sans: the primary proportional (UI body) face, and a fallback for the
    // "display" family so any glyph the serif lacks still renders.
    ctx.add_font(FontInsert::new(
        "noto_sans",
        FontData::from_static(include_bytes!("../fonts/NotoSans-Regular.ttf")),
        vec![
            fam(Proportional, Highest),
            fam(Name("display".into()), Lowest),
        ],
    ));

    // Noto Sans Symbols2: glyph/symbol fallback.
    ctx.add_font(FontInsert::new(
        "noto_symbols",
        FontData::from_static(include_bytes!("../fonts/NotoSansSymbols2-Regular.ttf")),
        vec![fam(Proportional, Lowest)],
    ));

    // IBM Plex Mono: measurement values render in this monospace face (tabular
    // figures line up in columns).
    ctx.add_font(FontInsert::new(
        "plex_mono",
        FontData::from_static(include_bytes!("../fonts/IBMPlexMono-Regular.ttf")),
        vec![fam(Monospace, Highest)],
    ));

    // IBM Plex Serif (SemiBold): the display face for headings and the wordmark,
    // registered under a custom "display" family.
    ctx.add_font(FontInsert::new(
        "plex_serif",
        FontData::from_static(include_bytes!("../fonts/IBMPlexSerif-SemiBold.ttf")),
        vec![fam(Name("display".into()), Highest)],
    ));
}

#[cfg(not(target_arch = "wasm32"))]
fn load_icon() -> egui::IconData {
    // macOS expects ~10% transparent margin around app icons so the system can
    // size and shadow them correctly. Other platforms render the icon as-is.
    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../assets/icon-256-mac.png").as_slice();
    #[cfg(not(target_os = "macos"))]
    let icon_bytes = include_bytes!("../assets/icon-256.png").as_slice();

    let img = image::load_from_memory(icon_bytes)
        .expect("Failed to load app icon")
        .into_rgba8();
    let (width, height) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    // Initialize tokio runtime for desktop
    let rt = tokio::runtime::Runtime::new().unwrap();
    let handle = rt.handle().clone();

    let icon = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("Print Junk")
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "Print Junk",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::PrintJunkApp::new(cc, handle)))
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
            "print_junk_canvas",
            web_options,
            Box::new(|cc| {
                setup_fonts(&cc.egui_ctx);
                theme::apply(&cc.egui_ctx);
                Ok(Box::new(app::PrintJunkApp::new(cc)))
            }),
        )
        .await
        .expect("Failed to start eframe");
}
