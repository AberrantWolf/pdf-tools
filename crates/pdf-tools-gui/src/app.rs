use eframe::egui;
use pdf_async_runtime::{DocumentId, PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

#[derive(Default, PartialEq)]
enum Mode {
    #[default]
    Viewer,
    Flashcards,
    Impose,
}

#[derive(Clone)]
struct ProgressState {
    operation: String,
    current: usize,
    total: usize,
}

struct ViewerState {
    current_doc_id: Option<DocumentId>,
    current_page: usize,
    total_pages: usize,
    page_texture: Option<egui::TextureHandle>,
}

pub struct PdfToolsApp {
    mode: Mode,
    csv_path: String,
    pdf_path: String,
    status: String,

    // Async infrastructure
    command_tx: mpsc::UnboundedSender<PdfCommand>,
    update_rx: mpsc::UnboundedReceiver<PdfUpdate>,

    // Progress tracking
    progress: Option<ProgressState>,

    // Viewer state
    viewer_state: Option<ViewerState>,

    // Runtime handle (native only)
    #[cfg(not(target_arch = "wasm32"))]
    _tokio_handle: tokio::runtime::Handle,
}

impl PdfToolsApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(_cc: &eframe::CreationContext<'_>, tokio_handle: tokio::runtime::Handle) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn worker task
        tokio_handle.spawn(crate::worker::worker_task(command_rx, update_tx));

        Self {
            mode: Mode::default(),
            csv_path: String::new(),
            pdf_path: String::new(),
            status: String::new(),
            command_tx,
            update_rx,
            progress: None,
            viewer_state: None,
            _tokio_handle: tokio_handle,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn worker task using wasm-bindgen-futures
        wasm_bindgen_futures::spawn_local(crate::worker::worker_task(command_rx, update_tx));

        Self {
            mode: Mode::default(),
            csv_path: String::new(),
            pdf_path: String::new(),
            status: String::new(),
            command_tx,
            update_rx,
            progress: None,
            viewer_state: None,
        }
    }
}

impl eframe::App for PdfToolsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle drag-and-drop for PDF files
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for file in &i.raw.dropped_files {
                    if let Some(path) = &file.path {
                        if path.extension().and_then(|s| s.to_str()) == Some("pdf") {
                            let _ = self.command_tx.send(PdfCommand::ViewerLoad {
                                path: path.clone(),
                            });
                            self.status = "Loading PDF...".to_string();
                        }
                    }
                }
            }
        });

        // Process all pending updates from worker
        while let Ok(update) = self.update_rx.try_recv() {
            match update {
                PdfUpdate::Progress {
                    operation,
                    current,
                    total,
                } => {
                    self.progress = Some(ProgressState {
                        operation,
                        current,
                        total,
                    });
                    ctx.request_repaint(); // Request another frame
                }
                PdfUpdate::FlashcardsLoaded { cards } => {
                    self.status = format!("Loaded {} flashcards from CSV", cards.len());
                    self.progress = None;
                    // Store cards for generation (simplified - directly generate)
                    let options = pdf_async_runtime::FlashcardOptions::default();
                    let _ = self.command_tx.send(PdfCommand::FlashcardsGenerate {
                        cards,
                        options,
                        output_path: "flashcards.pdf".into(),
                    });
                }
                PdfUpdate::FlashcardsComplete { path, card_count } => {
                    self.status = format!("Generated {} flashcards → {}", card_count, path.display());
                    self.progress = None;
                }
                PdfUpdate::ImposeLoaded { doc_id, page_count } => {
                    self.status = format!("Loaded PDF with {} pages (ID: {:?})", page_count, doc_id);
                    self.progress = None;
                }
                PdfUpdate::ImposeComplete { path } => {
                    self.status = format!("Imposed PDF → {}", path.display());
                    self.progress = None;
                }
                PdfUpdate::Error { message } => {
                    self.status = format!("Error: {message}");
                    self.progress = None;
                }
                PdfUpdate::ViewerLoaded { doc_id, page_count } => {
                    self.viewer_state = Some(ViewerState {
                        current_doc_id: Some(doc_id),
                        current_page: 0,
                        total_pages: page_count,
                        page_texture: None,
                    });
                    self.status = format!("Loaded PDF with {} pages", page_count);
                    self.progress = None;

                    // Request render of first page
                    let _ = self.command_tx.send(PdfCommand::ViewerRenderPage {
                        doc_id,
                        page_index: 0,
                    });
                }
                PdfUpdate::ViewerPageRendered {
                    rgba_data,
                    width,
                    height,
                    ..
                } => {
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(
                        [width, height],
                        &rgba_data,
                    );

                    if let Some(state) = &mut self.viewer_state {
                        if let Some(texture) = &mut state.page_texture {
                            texture.set(color_image, egui::TextureOptions::default());
                        } else {
                            state.page_texture = Some(
                                ctx.load_texture(
                                    "pdf_page",
                                    color_image,
                                    egui::TextureOptions::default(),
                                ),
                            );
                        }
                    }
                    self.progress = None;
                }
                PdfUpdate::ViewerClosed { .. } => {
                    self.viewer_state = None;
                    self.status = "Closed PDF".to_string();
                }
            }
        }

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

            // Show progress bar
            if let Some(ref progress) = self.progress {
                ui.separator();
                ui.label(&progress.operation);
                ui.add(
                    egui::ProgressBar::new(
                        progress.current as f32 / progress.total.max(1) as f32,
                    )
                    .show_percentage(),
                );
                ctx.request_repaint(); // Keep updating during operations
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
        if let Some(state) = &mut self.viewer_state {
            // Show navigation bar
            ui.horizontal(|ui| {
                let can_go_back = state.current_page > 0;
                let can_go_forward = state.current_page < state.total_pages.saturating_sub(1);

                if ui
                    .add_enabled(can_go_back, egui::Button::new("◀ Previous"))
                    .clicked()
                {
                    state.current_page -= 1;
                    if let Some(doc_id) = state.current_doc_id {
                        let _ = self.command_tx.send(PdfCommand::ViewerRenderPage {
                            doc_id,
                            page_index: state.current_page,
                        });
                        self.status = format!("Rendering page {}...", state.current_page + 1);
                    }
                }

                ui.label(format!(
                    "Page {} of {}",
                    state.current_page + 1,
                    state.total_pages
                ));

                if ui
                    .add_enabled(can_go_forward, egui::Button::new("Next ▶"))
                    .clicked()
                {
                    state.current_page += 1;
                    if let Some(doc_id) = state.current_doc_id {
                        let _ = self.command_tx.send(PdfCommand::ViewerRenderPage {
                            doc_id,
                            page_index: state.current_page,
                        });
                        self.status = format!("Rendering page {}...", state.current_page + 1);
                    }
                }

                ui.separator();

                if ui.button("Close PDF").clicked() {
                    if let Some(doc_id) = state.current_doc_id {
                        let _ = self.command_tx.send(PdfCommand::ViewerClose { doc_id });
                    }
                }
            });

            ui.separator();

            // Display page texture if available
            if let Some(texture) = &state.page_texture {
                // Center the image
                egui::ScrollArea::both().show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.image((texture.id(), texture.size_vec2()));
                    });
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                    ui.label("Rendering page...");
                });
            }

            // TODO: Add zoom controls
            // TODO: Add jump to page input
            // TODO: Add thumbnail sidebar
        } else {
            // No PDF loaded - show file loading UI
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.heading("PDF Viewer");
                ui.add_space(20.0);

                #[cfg(feature = "pdf-viewer")]
                {
                    ui.label("Drop a PDF file here or click to open");
                    ui.add_space(10.0);

                    if ui.button("Open PDF...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .pick_file()
                        {
                            let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                            self.status = "Loading PDF...".to_string();
                        }
                    }
                }

                #[cfg(not(feature = "pdf-viewer"))]
                {
                    ui.label("PDF viewing not available in WASM build");
                }
            });
        }
    }

    fn show_flashcards(&mut self, ui: &mut egui::Ui) {
        ui.heading("Generate Flashcards");

        ui.horizontal(|ui| {
            ui.label("CSV file:");
            ui.text_edit_singleline(&mut self.csv_path);
            if ui.button("Browse...").clicked() {
                // File dialog (to be implemented)
            }
        });

        if ui.button("Generate PDF").clicked() {
            // Send command to worker instead of blocking
            let _ = self.command_tx.send(PdfCommand::FlashcardsLoadCsv {
                input_path: self.csv_path.clone().into(),
            });
            self.status = "Loading CSV...".to_string();
        }
    }

    fn show_impose(&mut self, ui: &mut egui::Ui) {
        ui.heading("PDF Imposition");

        ui.horizontal(|ui| {
            ui.label("PDF file:");
            ui.text_edit_singleline(&mut self.pdf_path);
        });

        ui.horizontal(|ui| {
            if ui.button("2-up").clicked() {
                let _ = self.command_tx.send(PdfCommand::ImposeLoad {
                    input_path: self.pdf_path.clone().into(),
                });
                self.status = "Loading PDF...".to_string();
            }
            if ui.button("4-up").clicked() {
                self.status = "4-up not yet implemented".to_string();
            }
            if ui.button("Booklet").clicked() {
                self.status = "Booklet not yet implemented".to_string();
            }
        });
    }
}
