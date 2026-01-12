use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

use super::state::ImposeState;

pub fn show(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            show_config_buttons(ui, state, command_tx);
        });

        ui.add_space(10.0);

        show_preview_button(ui, state, command_tx);

        ui.add_space(5.0);

        show_generate_button(ui, state, command_tx);

        if state.needs_regeneration && !state.options.input_files.is_empty() {
            generate_preview(state, command_tx);
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn show_config_buttons(
    ui: &mut egui::Ui,
    state: &ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    if ui.button("💾 Save Configuration").clicked() {
        save_configuration(state);
    }

    if ui.button("📂 Load Configuration").clicked() {
        load_configuration(command_tx);
    }
}

#[cfg(target_arch = "wasm32")]
fn show_config_buttons(
    _ui: &mut egui::Ui,
    _state: &ImposeState,
    _command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
}

#[cfg(not(target_arch = "wasm32"))]
fn save_configuration(state: &ImposeState) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .set_file_name("impose_config.json")
        .save_file()
    {
        let options = state.options.clone();
        tokio::spawn(async move {
            if let Err(e) = options.save(&path).await {
                log::error!("Failed to save configuration: {}", e);
            } else {
                log::info!("Configuration saved to {}", path.display());
            }
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_configuration(command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("JSON", &["json"])
        .pick_file()
    {
        let _ = command_tx.send(PdfCommand::ImposeLoadConfig { path });
    }
}

fn show_preview_button(
    ui: &mut egui::Ui,
    state: &mut ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let can_generate = !state.options.input_files.is_empty();

    if ui
        .add_enabled(can_generate, egui::Button::new("📄 Generate Preview"))
        .clicked()
    {
        generate_preview(state, command_tx);
    }
}

fn generate_preview(state: &mut ImposeState, command_tx: &mpsc::UnboundedSender<PdfCommand>) {
    state.needs_regeneration = false;
    log::info!("Generating impose preview");
    let _ = command_tx.send(PdfCommand::ImposeGenerate {
        options: state.options.clone(),
        output_path: std::env::temp_dir().join("impose_preview.pdf"),
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn show_generate_button(
    ui: &mut egui::Ui,
    state: &ImposeState,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    let can_generate = !state.options.input_files.is_empty();

    if ui
        .add_enabled(can_generate, egui::Button::new("💾 Save PDF..."))
        .clicked()
    {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .set_file_name("imposed.pdf")
            .save_file()
        {
            log::info!("Saving imposed PDF to: {}", path.display());
            let _ = command_tx.send(PdfCommand::ImposeGenerate {
                options: state.options.clone(),
                output_path: path,
            });
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn show_generate_button(
    _ui: &mut egui::Ui,
    _state: &ImposeState,
    _command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
}
