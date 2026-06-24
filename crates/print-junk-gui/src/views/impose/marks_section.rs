use eframe::egui;

use super::state::ImposeState;
use crate::ui_components::{form, form_row, form_row_info, num_field, section};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    section(ui, "imp_marks_sec", "Printer's marks", false, |ui| {
        let mut changed = false;

        changed |= ui
            .checkbox(&mut state.options.marks.fold_lines, "Fold lines")
            .on_hover_text(
                "Dashed lines indicating where to fold the sheet, including the spine fold",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut state.options.marks.trim_marks,
                "Trim marks (guillotine)",
            )
            .on_hover_text(
                "L-shaped marks at fold edges showing where to trim after folding and binding",
            )
            .changed();
        changed |= ui
            .checkbox(
                &mut state.options.marks.crop_marks,
                "Crop marks (sheet edges)",
            )
            .on_hover_text("Corner marks at the sheet edges for trimming")
            .changed();
        changed |= ui
            .checkbox(
                &mut state.options.marks.registration_marks,
                "Registration marks",
            )
            .on_hover_text("Crosshair marks for aligning front and back printing")
            .changed();

        // Sewing and collation marks only apply to signature-based bindings
        if state.options.binding_type.uses_signatures() {
            ui.separator();

            changed |= ui
                .checkbox(
                    &mut state.options.marks.sewing_marks,
                    "Sewing station marks",
                )
                .on_hover_text("Marks on the spine indicating where to pierce for sewing")
                .changed();

            // Show sewing configuration when sewing marks are enabled
            if state.options.marks.sewing_marks {
                let cfg = &mut state.options.sewing_config;
                form(ui, "imp_sewing", |ui| {
                    changed |= form_row(ui, "Stations", |ui| {
                        num_field(ui, &mut cfg.station_count, 1..=10, "")
                    });
                    changed |= form_row_info(
                        ui,
                        "Kettle offset",
                        "Distance from spine to the outermost sewing station",
                        |ui| num_field(ui, &mut cfg.kettle_offset_mm, 5.0..=30.0, " mm"),
                    );
                });
            }

            changed |= ui
                .checkbox(
                    &mut state.options.marks.collation_marks,
                    "Collation marks (back marks)",
                )
                .on_hover_text("Marks on the spine to verify signature order during assembly")
                .changed();
        }

        // Appearance controls
        let has_interior = state.options.marks.fold_lines
            || state.options.marks.trim_marks
            || state.options.marks.sewing_marks;
        let has_exterior = state.options.marks.crop_marks
            || state.options.marks.registration_marks
            || state.options.marks.collation_marks;

        if has_interior || has_exterior {
            ui.separator();
        }

        if has_interior {
            ui.label("Interior marks (fold, trim, sewing):")
                .on_hover_text(
                    "Marks near fold/trim edges that may be visible in the finished book",
                );
            ui.indent("interior_appearance", |ui| {
                changed |=
                    show_appearance_controls(ui, &mut state.options.interior_marks_appearance);
            });
        }

        if has_exterior {
            ui.label("Exterior marks (crop, registration, collation):")
                .on_hover_text("Marks at sheet edges, reliably trimmed or covered by binding");
            ui.indent("exterior_appearance", |ui| {
                changed |=
                    show_appearance_controls(ui, &mut state.options.exterior_marks_appearance);
            });
        }

        if changed {
            state.needs_regeneration = true;
        }
    });
}

fn show_appearance_controls(
    ui: &mut egui::Ui,
    appearance: &mut pdf_impose::MarksAppearance,
) -> bool {
    let mut changed = false;

    let prev_gray = appearance.gray;
    ui.horizontal(|ui| {
        ui.label("Gray:");
        ui.add(
            egui::Slider::new(&mut appearance.gray, 0.0..=1.0)
                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
        );
    });
    if (appearance.gray - prev_gray).abs() > f32::EPSILON {
        changed = true;
    }

    let prev_scale = appearance.line_width_scale;
    ui.horizontal(|ui| {
        ui.label("Line weight:");
        ui.add(egui::Slider::new(
            &mut appearance.line_width_scale,
            0.1..=4.0,
        ));
    });
    if (appearance.line_width_scale - prev_scale).abs() > f32::EPSILON {
        changed = true;
    }

    changed
}
