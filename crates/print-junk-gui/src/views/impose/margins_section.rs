use eframe::egui;
use pdf_impose::CreepConfig;

use super::state::ImposeState;
use crate::ui_components::{form, form_row, form_row_info, num_field, section};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    section(ui, "imp_margins_sec", "Margins", false, |ui| {
        let mut changed = false;

        ui.label("Sheet margins (printer-safe area)")
            .on_hover_text("Margins inside the printable area of the physical sheet");
        {
            let s = &mut state.options.margins.sheet;
            form(ui, "imp_sheet_margins", |ui| {
                changed |= form_row(ui, "Top", |ui| {
                    num_field(ui, &mut s.top_mm, 0.0..=25.0, " mm")
                });
                changed |= form_row(ui, "Bottom", |ui| {
                    num_field(ui, &mut s.bottom_mm, 0.0..=25.0, " mm")
                });
                changed |= form_row(ui, "Left", |ui| {
                    num_field(ui, &mut s.left_mm, 0.0..=25.0, " mm")
                });
                changed |= form_row(ui, "Right", |ui| {
                    num_field(ui, &mut s.right_mm, 0.0..=25.0, " mm")
                });
            });
        }

        ui.add_space(8.0);

        ui.label("Leaf margins (trim & gutter)")
            .on_hover_text("Margins around each book page within its cell on the sheet");
        {
            let l = &mut state.options.margins.leaf;
            form(ui, "imp_leaf_margins", |ui| {
                changed |=
                    form_row_info(ui, "Head", "Margin at the top (head) of the page", |ui| {
                        num_field(ui, &mut l.top_mm, 0.0..=50.0, " mm")
                    });
                changed |= form_row_info(
                    ui,
                    "Foot",
                    "Margin at the bottom (tail/foot) of the page",
                    |ui| num_field(ui, &mut l.bottom_mm, 0.0..=50.0, " mm"),
                );
                changed |= form_row_info(
                    ui,
                    "Fore-edge",
                    "Margin on the side opposite the spine",
                    |ui| num_field(ui, &mut l.fore_edge_mm, 0.0..=50.0, " mm"),
                );
                changed |= form_row_info(
                    ui,
                    "Spine",
                    "Margin at the spine, where pages are bound together",
                    |ui| num_field(ui, &mut l.spine_mm, 0.0..=50.0, " mm"),
                );
                changed |= form_row_info(
                    ui,
                    "Trim",
                    "Extra material around fold edges, trimmed away after binding (3 mm standard)",
                    |ui| num_field(ui, &mut l.trim_allowance_mm, 0.0..=50.0, " mm"),
                );
            });
        }

        // Creep compensation (only for signature-based bindings)
        if state.options.binding_type.uses_signatures() {
            ui.add_space(8.0);
            changed |= show_creep_section(ui, state);
        }

        if changed {
            state.needs_regeneration = true;
        }
    });
}

fn show_creep_section(ui: &mut egui::Ui, state: &mut ImposeState) -> bool {
    let mut changed = false;

    ui.label("Creep compensation:").on_hover_text(
        "Compensates for paper thickness in folded signatures. \
             Inner sheets protrude at the fore edge; creep shifts \
             their content toward the spine so margins stay even after trimming.",
    );

    ui.indent("creep_config", |ui| {
        // Mode selector
        ui.horizontal(|ui| {
            if ui
                .selectable_label(matches!(state.options.creep, CreepConfig::None), "None")
                .clicked()
            {
                state.options.creep = CreepConfig::None;
                changed = true;
            }

            if ui
                .selectable_label(
                    matches!(state.options.creep, CreepConfig::PerLayer { .. }),
                    "Per layer",
                )
                .on_hover_text("Fixed offset per nested leaf layer")
                .clicked()
            {
                state.options.creep = CreepConfig::PerLayer {
                    creep_per_layer_mm: 0.1,
                };
                changed = true;
            }

            if ui
                .selectable_label(
                    matches!(state.options.creep, CreepConfig::FromCaliper { .. }),
                    "From caliper",
                )
                .on_hover_text(
                    "Computed from paper caliper using fold geometry \
                     (e.g., 80gsm ≈ 0.10 mm, 120gsm ≈ 0.14 mm)",
                )
                .clicked()
            {
                state.options.creep = CreepConfig::FromCaliper {
                    paper_thickness_mm: 0.1,
                };
                changed = true;
            }
        });

        // Mode-specific controls
        match &mut state.options.creep {
            CreepConfig::None => {}
            CreepConfig::PerLayer { creep_per_layer_mm } => {
                changed |= ui
                    .horizontal(|ui| {
                        ui.label("Per layer:");
                        ui.add(
                            egui::DragValue::new(creep_per_layer_mm)
                                .range(0.01..=2.0)
                                .speed(0.01)
                                .suffix(" mm"),
                        )
                        .changed()
                    })
                    .inner;
            }
            CreepConfig::FromCaliper { paper_thickness_mm } => {
                changed |= ui
                    .horizontal(|ui| {
                        ui.label("Paper caliper:");
                        ui.add(
                            egui::DragValue::new(paper_thickness_mm)
                                .range(0.01..=0.5)
                                .speed(0.01)
                                .suffix(" mm"),
                        )
                        .changed()
                    })
                    .inner;
            }
        }

        // Info line: max creep offset and spine margin check
        if state.options.creep.is_enabled() {
            let max_creep = pdf_impose::max_creep_offset_mm(
                state.options.creep,
                state.options.page_arrangement,
                state.options.sheets_per_signature,
            );
            let spine_mm = state.options.margins.leaf.spine_mm;

            ui.add_space(4.0);
            ui.label(format!("Max creep offset: {max_creep:.2} mm"));

            if max_creep > spine_mm {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 180, 50),
                    format!(
                        "Spine margin ({spine_mm:.1} mm) is less than max creep ({max_creep:.2} mm)"
                    ),
                );

                if ui
                    .button(format!("Set spine margin to {max_creep:.2} mm"))
                    .on_hover_text(
                        "Increase the spine (gutter) margin so it absorbs the maximum \
                         creep shift. Without this, the innermost leaves' content may \
                         cross the spine fold.",
                    )
                    .clicked()
                {
                    state.options.margins.leaf.spine_mm = max_creep;
                    changed = true;
                }
            }
        }
    });

    changed
}
