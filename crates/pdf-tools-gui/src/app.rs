use eframe::egui;
use pdf_async_runtime::{PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

use crate::logger::AppLogger;
use crate::views::{
    FlashcardState, ImposeState, ViewerState, show_flashcards, show_impose, show_viewer,
};

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

    // Logging
    logger: AppLogger,
    log_viewer_open: bool,

    // Async infrastructure
    command_tx: mpsc::UnboundedSender<PdfCommand>,
    update_rx: mpsc::UnboundedReceiver<PdfUpdate>,

    // Progress tracking
    progress: Option<ProgressState>,

    // Feature state
    flashcard_state: FlashcardState,
    viewer_state: Option<ViewerState>,
    impose_state: ImposeState,

    // Runtime handle (native only)
    #[cfg(not(target_arch = "wasm32"))]
    _tokio_handle: tokio::runtime::Handle,
}

impl PdfToolsApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(_cc: &eframe::CreationContext<'_>, tokio_handle: tokio::runtime::Handle) -> Self {
        let logger = AppLogger::new(1000);
        logger.clone().init().expect("Failed to initialize logger");

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn worker task
        tokio_handle.spawn(crate::worker::worker_task(command_rx, update_tx));

        log::info!("PDF Tools GUI started");

        Self {
            mode: Mode::default(),
            logger,
            log_viewer_open: false,
            command_tx,
            update_rx,
            progress: None,
            flashcard_state: FlashcardState::default(),
            viewer_state: None,
            impose_state: ImposeState::default(),
            _tokio_handle: tokio_handle,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let logger = AppLogger::new(1000);
        logger.clone().init().expect("Failed to initialize logger");

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Spawn worker task using wasm-bindgen-futures
        wasm_bindgen_futures::spawn_local(crate::worker::worker_task(command_rx, update_tx));

        log::info!("PDF Tools GUI started");

        Self {
            mode: Mode::default(),
            logger,
            log_viewer_open: false,
            command_tx,
            update_rx,
            progress: None,
            flashcard_state: FlashcardState::default(),
            viewer_state: None,
            impose_state: ImposeState::default(),
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
                            log::info!("Loading PDF: {}", path.display());
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
                    log::info!("Loaded {} flashcards from CSV", cards.len());
                    self.progress = None;
                    self.flashcard_state.cards = cards;
                }
                PdfUpdate::FlashcardsComplete { path, card_count } => {
                    log::info!("Generated {} flashcards → {}", card_count, path.display());
                    self.progress = None;

                    // Load preview if it's a temp file
                    if path.starts_with(std::env::temp_dir()) {
                        let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                    }
                }
                PdfUpdate::ImposeLoaded { doc_id, page_count } => {
                    log::info!("Loaded PDF with {} pages (ID: {:?})", page_count, doc_id);
                    self.progress = None;
                }
                PdfUpdate::ImposeComplete { path } => {
                    log::info!("Imposed PDF → {}", path.display());
                    self.progress = None;

                    // Load preview if it's a temp file
                    if path.starts_with(std::env::temp_dir()) {
                        let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                    }
                }
                PdfUpdate::ImposePreviewGenerated { doc_id, page_count } => {
                    log::info!("Preview generated with {} pages", page_count);
                    self.impose_state.preview_doc_id = Some(doc_id);
                    self.impose_state.preview_page_count = page_count;
                    self.progress = None;

                    // Request render of first page
                    let _ = self.command_tx.send(PdfCommand::ViewerRenderPage {
                        doc_id,
                        page_index: 0,
                    });
                }
                PdfUpdate::ImposeConfigLoaded { options } => {
                    log::info!("Configuration loaded");
                    self.impose_state.options = options.clone();
                    self.progress = None;

                    // Recalculate stats with new options
                    let _ = self
                        .command_tx
                        .send(PdfCommand::ImposeCalculateStats { options });
                }
                PdfUpdate::ImposeStatsCalculated { stats } => {
                    self.impose_state.stats = Some(stats);
                }
                PdfUpdate::Error { message } => {
                    log::error!("Error: {}", message);
                    self.progress = None;
                }
                PdfUpdate::ViewerLoaded { doc_id, page_count } => {
                    let new_viewer_state = ViewerState {
                        current_doc_id: Some(doc_id),
                        current_page: 0,
                        total_pages: page_count,
                        page_texture: None,
                    };

                    // Update viewer state based on current mode
                    match self.mode {
                        Mode::Flashcards => {
                            self.flashcard_state.preview_viewer = Some(new_viewer_state.clone());
                        }
                        Mode::Viewer => {
                            self.viewer_state = Some(new_viewer_state.clone());
                        }
                        Mode::Impose => {
                            self.impose_state.preview_viewer = Some(new_viewer_state.clone());
                        }
                    }

                    log::info!("Loaded PDF with {} pages", page_count);
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

                    // Update the appropriate viewer state
                    if let Some(state) = &mut self.viewer_state {
                        if let Some(texture) = &mut state.page_texture {
                            texture.set(color_image.clone(), egui::TextureOptions::default());
                        } else {
                            state.page_texture = Some(ctx.load_texture(
                                "pdf_page",
                                color_image.clone(),
                                egui::TextureOptions::default(),
                            ));
                        }
                    }

                    if let Some(state) = &mut self.flashcard_state.preview_viewer {
                        if let Some(texture) = &mut state.page_texture {
                            texture.set(color_image.clone(), egui::TextureOptions::default());
                        } else {
                            state.page_texture = Some(ctx.load_texture(
                                "flashcard_preview",
                                color_image.clone(),
                                egui::TextureOptions::default(),
                            ));
                        }
                    }

                    if let Some(state) = &mut self.impose_state.preview_viewer {
                        if let Some(texture) = &mut state.page_texture {
                            texture.set(color_image.clone(), egui::TextureOptions::default());
                        } else {
                            state.page_texture = Some(ctx.load_texture(
                                "impose_preview",
                                color_image,
                                egui::TextureOptions::default(),
                            ));
                        }
                    }

                    self.progress = None;
                }
                PdfUpdate::ViewerClosed { .. } => {
                    self.viewer_state = None;
                    log::info!("Closed PDF");
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

        // Status bar at bottom
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Show progress bar
                if let Some(ref progress) = self.progress {
                    ui.label(&progress.operation);
                    ui.add(
                        egui::ProgressBar::new(
                            progress.current as f32 / progress.total.max(1) as f32,
                        )
                        .show_percentage(),
                    );
                    ctx.request_repaint(); // Keep updating during operations
                } else if let Some(latest) = self.logger.latest_message() {
                    if ui.link(&latest).clicked() {
                        self.log_viewer_open = true;
                    }
                }
            });
        });

        // Log viewer window
        egui::Window::new("Log Viewer")
            .open(&mut self.log_viewer_open)
            .default_size([800.0, 400.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Application Logs");
                    if ui.button("Clear").clicked() {
                        self.logger.clear();
                    }
                });

                ui.separator();

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let entries = self.logger.get_entries();

                        for entry in entries.iter().rev() {
                            ui.horizontal(|ui| {
                                // Timestamp
                                ui.label(
                                    egui::RichText::new(
                                        entry.timestamp.format("%H:%M:%S%.3f").to_string(),
                                    )
                                    .monospace()
                                    .color(egui::Color32::GRAY),
                                );

                                // Level with color
                                let (level_text, level_color) = match entry.level {
                                    log::Level::Error => {
                                        ("ERROR", egui::Color32::from_rgb(255, 80, 80))
                                    }
                                    log::Level::Warn => {
                                        ("WARN ", egui::Color32::from_rgb(255, 200, 80))
                                    }
                                    log::Level::Info => {
                                        ("INFO ", egui::Color32::from_rgb(80, 200, 255))
                                    }
                                    log::Level::Debug => {
                                        ("DEBUG", egui::Color32::from_rgb(200, 200, 200))
                                    }
                                    log::Level::Trace => {
                                        ("TRACE", egui::Color32::from_rgb(150, 150, 150))
                                    }
                                };

                                ui.label(
                                    egui::RichText::new(level_text)
                                        .monospace()
                                        .color(level_color),
                                );

                                // Module
                                ui.label(
                                    egui::RichText::new(&entry.target)
                                        .monospace()
                                        .color(egui::Color32::from_rgb(150, 150, 255)),
                                );

                                // Message
                                ui.label(&entry.message);
                            });
                        }
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| match self.mode {
            Mode::Viewer => show_viewer(ui, &mut self.viewer_state, &self.command_tx),
            Mode::Flashcards => show_flashcards(ui, &mut self.flashcard_state, &self.command_tx),
            Mode::Impose => show_impose(ui, &mut self.impose_state, &self.command_tx),
        });
    }
}
