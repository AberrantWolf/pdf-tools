use eframe::egui;
use pdf_async_runtime::PdfCommand;
use tokio::sync::mpsc;

pub fn show_impose(
    ui: &mut egui::Ui,
    pdf_path: &mut String,
    command_tx: &mpsc::UnboundedSender<PdfCommand>,
) {
    ui.heading("PDF Imposition");

    ui.horizontal(|ui| {
        ui.label("PDF file:");
        ui.text_edit_singleline(pdf_path);
    });

    ui.horizontal(|ui| {
        if ui.button("2-up").clicked() {
            log::info!("Loading PDF for 2-up imposition: {}", pdf_path);
            let _ = command_tx.send(PdfCommand::ImposeLoad {
                input_path: pdf_path.clone().into(),
            });
        }
        if ui.button("4-up").clicked() {
            log::warn!("4-up not yet implemented");
        }
        if ui.button("Booklet").clicked() {
            log::warn!("Booklet not yet implemented");
        }
    });
}
