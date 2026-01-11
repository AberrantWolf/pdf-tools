use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

pub fn show_flashcards(
    ui: &mut egui::Ui,
    csv_path: &mut String,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
    status: &mut String,
) {
    ui.heading("Generate Flashcards");

    ui.horizontal(|ui| {
        ui.label("CSV file:");
        ui.text_edit_singleline(csv_path);
        if ui.button("Browse...").clicked() {
            // File dialog (to be implemented)
        }
    });

    if ui.button("Generate PDF").clicked() {
        // Send command to worker instead of blocking
        let _ = command_tx.send(PdfCommand::FlashcardsLoadCsv {
            input_path: csv_path.clone().into(),
        });
        *status = "Loading CSV...".to_string();
    }
}
