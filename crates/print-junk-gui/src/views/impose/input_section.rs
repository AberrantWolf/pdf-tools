use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

use super::state::ImposeState;
use crate::ui_components::{FileListEditor, section};

pub fn show(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    _command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    section(ui, "imp_input_sec", "Input files", true, |ui| {
        if ui.button("➕ Add PDF Files").clicked() {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(paths) = rfd::FileDialog::new()
                .add_filter("PDF", &["pdf"])
                .pick_files()
            {
                for path in paths {
                    if !state.options.input_files.contains(&path) {
                        state.options.input_files.push(path.clone());
                        state.needs_regeneration = true;
                    }
                }
            }
        }

        ui.add_space(5.0);

        if FileListEditor::new(&mut state.options.input_files).show(ui) {
            if state.options.input_files.is_empty() {
                state.preview_viewer = None;
                state.preview_page_count = 0;
                state.preview_signatures_shown = None;
                state.preview_total_signatures = None;
                state.stats = None;
            } else {
                state.needs_regeneration = true;
            }
        }
    });
}
