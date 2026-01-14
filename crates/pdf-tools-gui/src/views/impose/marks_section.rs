use eframe::egui;
use pdf_impose::BindingType;

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
                .checkbox(&mut state.options.marks.crop_marks, "Crop marks")
                .changed();
            changed |= ui
                .checkbox(
                    &mut state.options.marks.registration_marks,
                    "Registration marks",
                )
                .changed();

            if is_signature_binding(&state.options.binding_type) {
                changed |= ui
                    .checkbox(&mut state.options.marks.sewing_marks, "Sewing marks")
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut state.options.marks.spine_marks,
                        "Spine marks (signature order)",
                    )
                    .changed();
            }

            if changed {
                state.needs_regeneration = true;
            }
        });
}

fn is_signature_binding(binding: &BindingType) -> bool {
    matches!(binding, BindingType::Signature | BindingType::CaseBinding)
}
