use eframe::egui;
use pdf_impose::{CascadeConfig, FlipAxis};

use super::state::ImposeState;
use crate::ui_components::{form, form_row, num_field, section};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    section(ui, "imp_cascade_sec", "Cascade", false, |ui| {
        ui.label("Tile multiple imposed sheets onto a single larger output page.")
            .on_hover_text(
                "When the paper size is large enough, multiple imposed sheets \
                 can be grouped on one page and cut apart after printing.",
            );
        ui.add_space(5.0);

        let cascade = state
            .options
            .cascade
            .get_or_insert_with(CascadeConfig::default);

        let mut changed = false;

        form(ui, "imp_cascade", |ui| {
            changed |= form_row(ui, "Columns", |ui| {
                num_field(ui, &mut cascade.cols, 1..=8, "")
            });
            changed |= form_row(ui, "Rows", |ui| num_field(ui, &mut cascade.rows, 1..=8, ""));
            changed |= form_row(ui, "Cell margin", |ui| {
                num_field(ui, &mut cascade.margin_mm, 0.0..=50.0, " mm")
            });
        });

        changed |= ui
            .checkbox(&mut cascade.cut_lines, "Cut lines between cells")
            .on_hover_text("Draw solid lines between cascade cells for guillotine cutting")
            .changed();

        // Duplex flip — a full-width pill group so both axes (and their tooltips)
        // stay visible.
        ui.horizontal(|ui| {
            ui.label("Duplex flip:");
            changed |= ui
                .selectable_value(&mut cascade.flip_axis, FlipAxis::LongEdge, "Long edge")
                .on_hover_text("Columns reverse on back side (standard)")
                .changed();
            changed |= ui
                .selectable_value(&mut cascade.flip_axis, FlipAxis::ShortEdge, "Short edge")
                .on_hover_text("Rows reverse on back side")
                .changed();
        });

        // Normalize trivial cascade to None (drops the `cascade` borrow above).
        if state
            .options
            .cascade
            .as_ref()
            .is_some_and(CascadeConfig::is_trivial)
        {
            state.options.cascade = None;
        }

        // Show derived cell dimensions.
        if state
            .options
            .cascade
            .as_ref()
            .is_some_and(|c| !c.is_trivial())
            && let Some((w, h)) = state.options.cell_dimensions_pt()
        {
            let w_mm = w / pdf_impose::constants::POINTS_PER_MM;
            let h_mm = h / pdf_impose::constants::POINTS_PER_MM;
            ui.add_space(5.0);
            ui.label(format!("Cell size: {w_mm:.1} × {h_mm:.1} mm"));
        }

        if changed {
            state.needs_regeneration = true;
        }
    });
}
