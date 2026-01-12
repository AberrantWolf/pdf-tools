mod actions_section;
mod additional_section;
mod binding_section;
mod input_section;
mod margins_section;
mod marks_section;
mod output_section;
mod state;
mod statistics_section;

pub use state::ImposeState;

use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

pub fn show_impose(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::SidePanel::left("impose_controls")
        .min_width(300.0)
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("PDF Imposition");
                ui.separator();

                input_section::show(ui, state, command_tx);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                binding_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                output_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                margins_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                marks_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                additional_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                statistics_section::show(ui, state);
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                actions_section::show(ui, state, command_tx);
            });
        });

    show_preview_area(ui, state, command_tx);
}

fn show_preview_area(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    egui::CentralPanel::default().show_inside(ui, |ui| {
        if state.preview_viewer.is_some() {
            super::show_viewer(ui, &mut state.preview_viewer, command_tx);
        } else if state.options.input_files.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("No Input Files");
                    ui.label("Add PDF files to begin");
                });
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Ready to Generate");
                    ui.label("Click 'Generate Preview' to see the imposed layout");
                });
            });
        }
    });
}
