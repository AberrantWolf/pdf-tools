use eframe::egui;

#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Viewer,
    Flashcards,
    Impose,
}

pub struct PdfToolsApp {
    mode: Mode,
    csv_path: String,
    pdf_path: String,
    status: String,
}

impl PdfToolsApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            mode: Mode::default(),
            csv_path: String::new(),
            pdf_path: String::new(),
            status: String::new(),
        }
    }
}

impl eframe::App for PdfToolsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.mode, Mode::Viewer, "📄 Viewer");
                ui.selectable_value(&mut self.mode, Mode::Flashcards, "🃏 Flashcards");
                ui.selectable_value(&mut self.mode, Mode::Impose, "📑 Impose");
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.mode {
                Mode::Viewer => self.show_viewer(ui),
                Mode::Flashcards => self.show_flashcards(ui),
                Mode::Impose => self.show_impose(ui),
            }

            if !self.status.is_empty() {
                ui.separator();
                ui.label(&self.status);
            }
        });
    }
}

impl PdfToolsApp {
    fn show_viewer(&mut self, ui: &mut egui::Ui) {
        ui.heading("PDF Viewer");

        #[cfg(feature = "pdf-viewer")]
        {
            ui.label("Drop a PDF file here or click to open");
            // PDF rendering implementation using pdfium-render
        }

        #[cfg(not(feature = "pdf-viewer"))]
        {
            ui.label("PDF viewing not available in WASM build");
        }
    }

    fn show_flashcards(&mut self, ui: &mut egui::Ui) {
        ui.heading("Generate Flashcards");

        ui.horizontal(|ui| {
            ui.label("CSV file:");
            ui.text_edit_singleline(&mut self.csv_path);
            if ui.button("Browse...").clicked() {
                // File dialog
            }
        });

        if ui.button("Generate PDF").clicked() {
            match pdf_flashcards::load_from_csv(&self.csv_path) {
                Ok(cards) => {
                    let options = pdf_flashcards::FlashcardOptions::default();
                    match pdf_flashcards::generate_pdf(&cards, &options, "flashcards.pdf") {
                        Ok(()) => self.status = format!("Generated {} flashcards", cards.len()),
                        Err(e) => self.status = format!("Error: {e}"),
                    }
                }
                Err(e) => self.status = format!("Error loading CSV: {e}"),
            }
        }
    }

    fn show_impose(&mut self, ui: &mut egui::Ui) {
        ui.heading("PDF Imposition");

        ui.horizontal(|ui| {
            ui.label("PDF file:");
            ui.text_edit_singleline(&mut self.pdf_path);
        });

        ui.horizontal(|ui| {
            if ui.button("2-up").clicked() { /* ... */ }
            if ui.button("4-up").clicked() { /* ... */ }
            if ui.button("Booklet").clicked() { /* ... */ }
        });
    }
}
