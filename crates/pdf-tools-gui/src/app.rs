use eframe::egui;
use pdf_async_runtime::{PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

use crate::views::{ViewerState, show_flashcards, show_impose, show_viewer};

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
                            let _ = self
                                .command_tx
                                .send(PdfCommand::ViewerLoad { path: path.clone() });
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
                    self.status =
                        format!("Generated {} flashcards → {}", card_count, path.display());
                    self.progress = None;
                }
                PdfUpdate::ImposeLoaded { doc_id, page_count } => {
                    self.status =
                        format!("Loaded PDF with {} pages (ID: {:?})", page_count, doc_id);
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
                    let color_image =
                        egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba_data);

                    if let Some(state) = &mut self.viewer_state {
                        if let Some(texture) = &mut state.page_texture {
                            texture.set(color_image, egui::TextureOptions::default());
                        } else {
                            state.page_texture = Some(ctx.load_texture(
                                "pdf_page",
                                color_image,
                                egui::TextureOptions::default(),
                            ));
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
                Mode::Viewer => show_viewer(
                    ui,
                    &mut self.viewer_state,
                    &self.command_tx,
                    &mut self.status,
                ),
                Mode::Flashcards => {
                    show_flashcards(ui, &mut self.csv_path, &self.command_tx, &mut self.status)
                }
                Mode::Impose => {
                    show_impose(ui, &mut self.pdf_path, &self.command_tx, &mut self.status)
                }
            }

            // Show progress bar
            if let Some(ref progress) = self.progress {
                ui.separator();
                ui.label(&progress.operation);
                ui.add(
                    egui::ProgressBar::new(progress.current as f32 / progress.total.max(1) as f32)
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
