use eframe::egui;

use super::state::ImposeState;
use crate::ui_components::PrintMarginsEditor;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("📏 Margins")
        .default_open(false)
        .show(ui, |ui| {
            if PrintMarginsEditor::new(
                &mut state.options.margins.top_mm,
                &mut state.options.margins.bottom_mm,
                &mut state.options.margins.fore_edge_mm,
                &mut state.options.margins.spine_mm,
                50.0,
            )
            .show(ui)
            {
                state.needs_regeneration = true;
            }
        });
}
