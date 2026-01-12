use eframe::egui;
use pdf_impose::{BindingType, SplitMode};

use super::state::ImposeState;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("⚙ Additional Options")
        .default_open(false)
        .show(ui, |ui| {
            if show_page_numbering(ui, state) {
                state.needs_regeneration = true;
            }
            ui.add_space(5.0);

            if show_flyleaves(ui, state) {
                state.needs_regeneration = true;
            }
            ui.add_space(5.0);

            if show_split_mode(ui, state) {
                state.needs_regeneration = true;
            }
        });
}

fn show_page_numbering(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        changed |= ui
            .checkbox(&mut state.options.add_page_numbers, "Add page numbers")
            .changed();

        if state.options.add_page_numbers {
            ui.label("Starting at:");
            changed |= ui
                .add(egui::DragValue::new(&mut state.options.page_number_start).range(1..=9999))
                .changed();
        }
    });

    changed
}

fn show_flyleaves(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.label("Front flyleaves:");
        changed |= ui
            .add(egui::DragValue::new(&mut state.options.front_flyleaves).range(0..=10))
            .changed();
    });

    ui.horizontal(|ui| {
        ui.label("Back flyleaves:");
        changed |= ui
            .add(egui::DragValue::new(&mut state.options.back_flyleaves).range(0..=10))
            .changed();
    });

    changed
}

fn show_split_mode(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    ui.label("Split output:");

    let changed_selector = show_split_mode_selector(ui, state);
    let changed_value = show_split_value_editor(ui, state);

    changed_selector || changed_value
}

fn show_split_mode_selector(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    let mut changed = false;

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

        if ui
            .selectable_label(
                matches!(state.options.split_mode, SplitMode::ByPages(_)),
                "By pages",
            )
            .clicked()
        {
            state.options.split_mode = SplitMode::ByPages(100);
            changed = true;
        }

        if ui
            .selectable_label(
                matches!(state.options.split_mode, SplitMode::BySheets(_)),
                "By sheets",
            )
            .clicked()
        {
            state.options.split_mode = SplitMode::BySheets(25);
            changed = true;
        }

        if is_signature_binding(&state.options.binding_type) {
            if ui
                .selectable_label(
                    matches!(state.options.split_mode, SplitMode::BySignatures(_)),
                    "By signatures",
                )
                .clicked()
            {
                state.options.split_mode = SplitMode::BySignatures(5);
                changed = true;
            }
        }
    });

    changed
}

fn show_split_value_editor(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    match &mut state.options.split_mode {
        SplitMode::ByPages(n) => {
            ui.horizontal(|ui| {
                ui.label("Pages per file:");
                ui.add(egui::DragValue::new(n).range(1..=1000)).changed()
            })
            .inner
        }
        SplitMode::BySheets(n) => {
            ui.horizontal(|ui| {
                ui.label("Sheets per file:");
                ui.add(egui::DragValue::new(n).range(1..=500)).changed()
            })
            .inner
        }
        SplitMode::BySignatures(n) => {
            ui.horizontal(|ui| {
                ui.label("Signatures per file:");
                ui.add(egui::DragValue::new(n).range(1..=100)).changed()
            })
            .inner
        }
        SplitMode::None => false,
    }
}

fn is_signature_binding(binding: &BindingType) -> bool {
    matches!(binding, BindingType::Signature | BindingType::CaseBinding)
}
