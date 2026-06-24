use eframe::egui;
use pdf_impose::{BindingType, PageArrangement};

use super::state::ImposeState;
use crate::ui_components::section;

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    section(ui, "imp_binding_sec", "Binding", true, |ui| {
        ui.label("Binding type:");
        if show_binding_type_selector(ui, &mut state.options.binding_type) {
            log::info!("Binding type changed to: {:?}", state.options.binding_type);
            state.needs_regeneration = true;
        }

        ui.add_space(5.0);

        if state.options.binding_type.uses_signatures()
            && show_arrangement_selector(
                ui,
                &mut state.options.page_arrangement,
                &mut state.options.sheets_per_signature,
            )
        {
            state.needs_regeneration = true;
        }
    });
}

fn show_binding_type_selector(ui: &mut egui::Ui, binding_type: &mut BindingType) -> bool {
    let mut changed = false;
    let options: &[(BindingType, &str, &str)] = &[
        (
            BindingType::Signature,
            "Signature",
            "Folded and sewn signatures, the traditional bookbinding method",
        ),
        (
            BindingType::PerfectBinding,
            "Perfect",
            "Pages glued directly to the spine",
        ),
        (
            BindingType::SideStitch,
            "Side Stitch",
            "Pages stapled or sewn through the side near the spine",
        ),
        (
            BindingType::Spiral,
            "Spiral",
            "Pages bound with a spiral or comb through punched holes",
        ),
        (
            BindingType::CaseBinding,
            "Case",
            "Signature binding with a rigid cover (hardcover)",
        ),
    ];
    ui.horizontal(|ui| {
        for (value, label, tooltip) in options {
            if ui
                .selectable_value(binding_type, *value, *label)
                .on_hover_text(*tooltip)
                .changed()
            {
                changed = true;
            }
        }
    });
    changed
}

fn show_arrangement_selector(
    ui: &mut egui::Ui,
    arrangement: &mut PageArrangement,
    sheets_per_signature: &mut usize,
) -> bool {
    let mut changed = false;

    ui.label("Page arrangement:");
    ui.horizontal(|ui| {
        let arrangements: &[(PageArrangement, &str, &str)] = &[
            (PageArrangement::Folio, "Folio", "1 fold, 4 pages per sheet"),
            (
                PageArrangement::Quarto,
                "Quarto",
                "2 folds, 8 pages per sheet",
            ),
            (
                PageArrangement::Octavo,
                "Octavo",
                "3 folds, 16 pages per sheet",
            ),
        ];
        for (value, label, tooltip) in arrangements {
            if ui
                .selectable_value(arrangement, *value, *label)
                .on_hover_text(*tooltip)
                .changed()
            {
                changed = true;
            }
        }
    });

    ui.horizontal(|ui| {
        ui.label("Sheets per signature:");
        let mut val = *sheets_per_signature as i32;
        if ui
            .add(egui::DragValue::new(&mut val).range(1..=16).speed(0.1))
            .changed()
        {
            *sheets_per_signature = (val.max(1) as usize).min(16);
            changed = true;
        }

        let pages_per_sig = arrangement.pages_per_sheet() * *sheets_per_signature;
        ui.weak(format!("({pages_per_sig} pages/signature)"));
    });

    changed
}
