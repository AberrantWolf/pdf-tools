use eframe::egui;

use super::state::ImposeState;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("✂ Printer's Marks")
        .default_open(false)
        .show(ui, |ui| {
            let mut changed = false;

            changed |= ui
                .checkbox(&mut state.options.marks.fold_lines, "Fold lines")
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.cut_lines,
                    "Cut lines (with scissors)",
                )
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.crop_marks,
                    "Crop marks (sheet edges)",
                )
                .changed();
            changed |= ui
                .checkbox(&mut state.options.marks.trim_marks, "Trim marks (per leaf)")
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.registration_marks,
                    "Registration marks",
                )
                .changed();

            if changed {
                state.needs_regeneration = true;
            }
        });
}
