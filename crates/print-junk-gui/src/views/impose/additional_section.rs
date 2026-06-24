use eframe::egui;
use pdf_impose::SplitMode;

use super::state::ImposeState;
use crate::ui_components::{form, form_row, form_row_info, num_field, section};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    section(ui, "imp_extras_sec", "Additional", false, |ui| {
        let mut changed = false;

        {
            let o = &mut state.options;
            form(ui, "imp_pagenum", |ui| {
                changed |= form_row(ui, "Page numbers", |ui| {
                    ui.checkbox(&mut o.add_page_numbers, "").changed()
                });
                if o.add_page_numbers {
                    changed |= form_row(ui, "Start at", |ui| {
                        num_field(ui, &mut o.page_number_start, 1..=9999, "")
                    });
                }
            });
        }

        if state.options.binding_type.uses_signatures() {
            let o = &mut state.options;
            form(ui, "imp_flyleaves", |ui| {
                changed |= form_row_info(
                    ui,
                    "Front flyleaves",
                    "Blank pages added at the front of the book",
                    |ui| num_field(ui, &mut o.front_flyleaves, 0..=10, ""),
                );
                changed |= form_row_info(
                    ui,
                    "Back flyleaves",
                    "Blank pages added at the back of the book",
                    |ui| num_field(ui, &mut o.back_flyleaves, 0..=10, ""),
                );
            });
        }

        changed |= show_split_mode(ui, state);

        if changed {
            state.needs_regeneration = true;
        }
    });
}

fn show_split_mode(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    ui.label("Split output:");
    let changed_selector = show_split_mode_selector(ui, state);
    let changed_value = show_split_value_editor(ui, state);
    changed_selector || changed_value
}

fn show_split_mode_selector(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    let mut changed = false;
    let supports_signatures = state.options.binding_type.uses_signatures();

    ui.horizontal(|ui| {
        if ui
            .selectable_label(
                matches!(state.options.split_mode, SplitMode::None),
                "No splitting",
            )
            .clicked()
        {
            state.options.split_mode = SplitMode::None;
            changed = true;
        }

        if supports_signatures
            && ui
                .selectable_label(
                    matches!(state.options.split_mode, SplitMode::BySignatures(_)),
                    "By signatures",
                )
                .clicked()
        {
            state.options.split_mode = SplitMode::BySignatures(1);
            changed = true;
        }
    });

    // Defensive: if the user switched to a non-signature binding while
    // BySignatures was selected, snap back to None so we never submit an
    // invalid configuration to the worker.
    if !supports_signatures && matches!(state.options.split_mode, SplitMode::BySignatures(_)) {
        state.options.split_mode = SplitMode::None;
        changed = true;
    }

    changed
}

fn show_split_value_editor(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    match &mut state.options.split_mode {
        SplitMode::BySignatures(n) => {
            let mut changed = false;
            form(ui, "imp_split_val", |ui| {
                changed |= form_row_info(
                    ui,
                    "Per file",
                    "1 produces one PDF per signature. \
                     Higher values group multiple signatures into each output file.",
                    |ui| num_field(ui, n, 1..=100, ""),
                );
            });
            changed
        }
        SplitMode::None => false,
    }
}
