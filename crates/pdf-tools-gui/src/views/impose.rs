use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

pub fn show_impose(
    ui: &mut egui::Ui,
    pdf_path: &mut String,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
    status: &mut String,
) {
    ui.heading("PDF Imposition");

    ui.horizontal(|ui| {
        ui.label("PDF file:");
        ui.text_edit_singleline(pdf_path);
    });

    ui.horizontal(|ui| {
        if ui.button("2-up").clicked() {
            let _ = command_tx.send(PdfCommand::ImposeLoad {
                input_path: pdf_path.clone().into(),
            });
            *status = "Loading PDF...".to_string();
        }
        if ui.button("4-up").clicked() {
            *status = "4-up not yet implemented".to_string();
        }
        if ui.button("Booklet").clicked() {
            *status = "Booklet not yet implemented".to_string();
        }
    });
}
