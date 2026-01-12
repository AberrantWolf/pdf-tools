use eframe::egui;
use pdf_impose::{BindingType, PageArrangement};

use super::state::ImposeState;
use crate::ui_components::button_group;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("📖 Binding & Arrangement")
        .default_open(true)
        .show(ui, |ui| {
            let binding_types = [
                (BindingType::Signature, "Signature"),
                (BindingType::PerfectBinding, "Perfect"),
                (BindingType::SideStitch, "Side Stitch"),
                (BindingType::Spiral, "Spiral"),
                (BindingType::CaseBinding, "Case"),
            ];

            ui.label("Binding type:");
            if button_group(ui, &mut state.options.binding_type, &binding_types) {
                log::info!("Binding type changed to: {:?}", state.options.binding_type);
                state.needs_regeneration = true;
            }

            ui.add_space(5.0);

            if is_signature_binding(&state.options.binding_type) {
                if show_arrangement_selector(ui, &mut state.options.page_arrangement) {
                    state.needs_regeneration = true;
                }
            }
        });
}

fn is_signature_binding(binding: &BindingType) -> bool {
    matches!(binding, BindingType::Signature | BindingType::CaseBinding)
}

fn show_arrangement_selector(ui: &mut egui::Ui, arrangement: &mut PageArrangement) -> bool {
    let mut changed = false;

    let arrangements = [
        (PageArrangement::Folio, "Folio (4pp)"),
        (PageArrangement::Quarto, "Quarto (8pp)"),
        (PageArrangement::Octavo, "Octavo (16pp)"),
    ];

    ui.label("Page arrangement:");
    changed |= button_group(ui, arrangement, &arrangements);

    if let PageArrangement::Custom {
        pages_per_signature,
    } = arrangement
    {
        ui.horizontal(|ui| {
            ui.label("Pages per signature:");
            changed |= ui
                .add(egui::DragValue::new(pages_per_signature).range(4..=256))
                .changed();
            ui.label("(must be multiple of 4)");
        });
    }

    if ui.button("Custom").clicked() {
        *arrangement = PageArrangement::Custom {
            pages_per_signature: 12,
        };
        changed = true;
    }

    changed
}
