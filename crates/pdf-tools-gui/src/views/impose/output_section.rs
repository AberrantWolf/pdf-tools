use eframe::egui;
use pdf_impose::{OutputFormat, PaperSize, Rotation, ScalingMode};

use super::state::ImposeState;
use crate::ui_components::{button_group, enum_selector};

pub fn show(ui: &mut egui::Ui, state: &mut ImposeState) {
    egui::CollapsingHeader::new("📐 Output Configuration")
        .default_open(true)
        .show(ui, |ui| {
            if show_paper_size_selector(ui, &mut state.options.output_paper_size) {
                state.needs_regeneration = true;
            }
            ui.add_space(5.0);

            if show_output_format_selector(ui, &mut state.options.output_format) {
                state.needs_regeneration = true;
            }
            ui.add_space(5.0);

            if show_scaling_mode_selector(ui, &mut state.options.scaling_mode) {
                state.needs_regeneration = true;
            }
            ui.add_space(5.0);

            if show_rotation_selector(ui, &mut state.options.source_rotation) {
                state.needs_regeneration = true;
            }
        });
}

fn show_paper_size_selector(ui: &mut egui::Ui, paper_size: &mut PaperSize) -> bool {
    let paper_sizes = [
        (PaperSize::Letter, "Letter"),
        (PaperSize::Legal, "Legal"),
        (PaperSize::Tabloid, "Tabloid"),
        (PaperSize::A3, "A3"),
        (PaperSize::A4, "A4"),
        (PaperSize::A5, "A5"),
    ];

    enum_selector(ui, "paper_size", "Paper size:", paper_size, &paper_sizes)
}

fn show_output_format_selector(ui: &mut egui::Ui, output_format: &mut OutputFormat) -> bool {
    let output_formats = [
        (OutputFormat::DoubleSided, "Double-sided (interleaved)"),
        (OutputFormat::TwoSided, "Two PDFs (front/back)"),
        (OutputFormat::SingleSidedSequence, "Single-sided sequence"),
    ];

    enum_selector(
        ui,
        "output_format",
        "Output format:",
        output_format,
        &output_formats,
    )
}

fn show_scaling_mode_selector(ui: &mut egui::Ui, scaling_mode: &mut ScalingMode) -> bool {
    let scaling_modes = [
        (ScalingMode::Fit, "Fit"),
        (ScalingMode::Fill, "Fill"),
        (ScalingMode::None, "None"),
        (ScalingMode::Stretch, "Stretch"),
    ];

    ui.label("Scaling mode:");
    button_group(ui, scaling_mode, &scaling_modes)
}

fn show_rotation_selector(ui: &mut egui::Ui, rotation: &mut Rotation) -> bool {
    let rotations = [
        (Rotation::None, "None"),
        (Rotation::Clockwise90, "90°"),
        (Rotation::Clockwise180, "180°"),
        (Rotation::Clockwise270, "270°"),
    ];

    ui.label("Source rotation:");
    button_group(ui, rotation, &rotations)
}
