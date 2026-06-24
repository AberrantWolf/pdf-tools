use eframe::egui;
use pdf_impose::{Orientation, OutputFormat, Rotation, ScalingMode};

use super::state::ImposeState;
use crate::ui_components::{STANDARD_PAPER_SIZES, enum_combo, form, form_row, section};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    let o = &mut state.options;
    let mut changed = false;

    let orientations = [
        (Orientation::Portrait, "Portrait"),
        (Orientation::Landscape, "Landscape"),
    ];
    let formats = [
        (OutputFormat::DoubleSided, "Double-sided (interleaved)"),
        (OutputFormat::TwoSided, "Two PDFs (front/back)"),
        (OutputFormat::SingleSidedSequence, "Single-sided sequence"),
    ];
    let scalings = [
        (ScalingMode::Fit, "Fit"),
        (ScalingMode::Fill, "Fill"),
        (ScalingMode::None, "None"),
        (ScalingMode::Stretch, "Stretch"),
    ];
    let rotations = [
        (Rotation::None, "None"),
        (Rotation::Clockwise90, "90°"),
        (Rotation::Clockwise180, "180°"),
        (Rotation::Clockwise270, "270°"),
    ];

    section(ui, "imp_output_sec", "Sheet & scaling", true, |ui| {
        form(ui, "imp_output", |ui| {
            changed |= form_row(ui, "Paper", |ui| {
                enum_combo(
                    ui,
                    "imp_paper",
                    &mut o.output_paper_size,
                    &STANDARD_PAPER_SIZES,
                )
            });
            changed |= form_row(ui, "Orientation", |ui| {
                enum_combo(ui, "imp_orient", &mut o.output_orientation, &orientations)
            });
            changed |= form_row(ui, "Format", |ui| {
                enum_combo(ui, "imp_format", &mut o.output_format, &formats)
            });
            changed |= form_row(ui, "Scaling", |ui| {
                enum_combo(ui, "imp_scaling", &mut o.scaling_mode, &scalings)
            });
            changed |= form_row(ui, "Rotation", |ui| {
                enum_combo(ui, "imp_rotation", &mut o.source_rotation, &rotations)
            });
        });
    });

    if changed {
        state.needs_regeneration = true;
    }
}
