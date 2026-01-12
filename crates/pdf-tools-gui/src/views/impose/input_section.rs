use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

use super::state::ImposeState;
use crate::ui_components::FileListEditor;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    _command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::CollapsingHeader::new("📄 Input Files")
        .default_open(true)
        .show(ui, |ui| {
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
                state.needs_regeneration = true;
            }
        });
}
