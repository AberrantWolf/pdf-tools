use eframe::egui;
use pdf_async_runtime::{PdfCommand, PdfUpdate};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use tokio::sync::mpsc;

use crate::logger::AppLogger;
use crate::startup::{Mode, StartupSettings};
use crate::views::{
    FlashcardState, ImposeState, ViewerState, show_flashcards, show_impose, show_viewer,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::views::{TypesettingState, show_typesetting};

/// eframe storage key for [`StartupSettings`].
const STARTUP_KEY: &str = "print_junk_startup";

/// eframe storage key for the auto-persisted project (all modes' settings).
#[cfg(not(target_arch = "wasm32"))]
const PROJECT_KEY: &str = "print_junk_project";

#[derive(Clone)]
struct ProgressState {
    operation: String,
    current: usize,
    total: usize,
}

pub struct PrintJunkApp {
    mode: Mode,

    // Startup selector
    startup: StartupSettings,
    show_startup: bool,

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
    #[cfg(not(target_arch = "wasm32"))]
    typesetting_state: TypesettingState,

    // Runtime handle (native only)
    #[cfg(not(target_arch = "wasm32"))]
    _tokio_handle: tokio::runtime::Handle,
}

impl PrintJunkApp {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(cc: &eframe::CreationContext<'_>, tokio_handle: tokio::runtime::Handle) -> Self {
        let logger = AppLogger::new(1000);
        logger.clone().init().expect("Failed to initialize logger");

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (worker_update_tx, mut worker_update_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Forward worker updates to the app channel, requesting repaint on each
        let repaint_ctx = cc.egui_ctx.clone();
        tokio_handle.spawn(async move {
            while let Some(update) = worker_update_rx.recv().await {
                if update_tx.send(update).is_err() {
                    break;
                }
                repaint_ctx.request_repaint();
            }
        });

        // Spawn worker task
        tokio_handle.spawn(crate::worker::worker_task(command_rx, worker_update_tx));

        log::info!("Print Junk GUI started");

        let startup = cc
            .storage
            .and_then(|s| eframe::get_value::<StartupSettings>(s, STARTUP_KEY))
            .unwrap_or_default();

        let mut app = Self {
            mode: startup.initial_mode(),
            show_startup: startup.should_show_on_launch(),
            startup,
            logger,
            log_viewer_open: false,
            command_tx,
            update_rx,
            progress: None,
            flashcard_state: FlashcardState::default(),
            viewer_state: None,
            impose_state: ImposeState::default(),
            typesetting_state: TypesettingState::default(),
            _tokio_handle: tokio_handle,
        };
        // Restore the last session's settings and re-load any referenced files.
        app.restore_session(cc.storage);
        app
    }

    #[cfg(target_arch = "wasm32")]
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let logger = AppLogger::new(1000);
        logger.clone().init().expect("Failed to initialize logger");

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (worker_update_tx, mut worker_update_rx) = mpsc::unbounded_channel();
        let (update_tx, update_rx) = mpsc::unbounded_channel();

        // Forward worker updates to the app channel, requesting repaint on each
        let repaint_ctx = cc.egui_ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            while let Some(update) = worker_update_rx.recv().await {
                if update_tx.send(update).is_err() {
                    break;
                }
                repaint_ctx.request_repaint();
            }
        });

        // Spawn worker task using wasm-bindgen-futures
        wasm_bindgen_futures::spawn_local(crate::worker::worker_task(command_rx, worker_update_tx));

        log::info!("Print Junk GUI started");

        let startup = cc
            .storage
            .and_then(|s| eframe::get_value::<StartupSettings>(s, STARTUP_KEY))
            .unwrap_or_default();

        Self {
            mode: startup.initial_mode(),
            show_startup: startup.should_show_on_launch(),
            startup,
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

    /// Embedded mode preview viewers. `PdfUpdate::ViewerPageRendered`/`ViewerClosed`
    /// are routed to whichever of these owns the matching document. Add a new
    /// mode's preview here and the rendering/close plumbing picks it up.
    fn preview_viewers_mut(&mut self) -> Vec<&mut Option<ViewerState>> {
        let mut viewers: Vec<&mut Option<ViewerState>> = vec![
            &mut self.flashcard_state.preview_viewer,
            &mut self.impose_state.preview_viewer,
        ];
        #[cfg(not(target_arch = "wasm32"))]
        viewers.push(&mut self.typesetting_state.preview_viewer);
        viewers
    }

    /// Restore the auto-persisted project from eframe storage on launch.
    #[cfg(not(target_arch = "wasm32"))]
    fn restore_session(&mut self, storage: Option<&dyn eframe::Storage>) {
        if let Some(project) =
            storage.and_then(|s| eframe::get_value::<crate::project::AppProject>(s, PROJECT_KEY))
        {
            self.apply_project(project);
        }
    }

    /// Replace every mode's settings with a loaded project, then re-load any
    /// referenced files from their stored paths (contents are never persisted).
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_project(&mut self, project: crate::project::AppProject) {
        // Flashcards: settings + re-load the CSV from its path.
        self.flashcard_state = project.flashcards;
        if !self.flashcard_state.csv_path.is_empty() {
            let _ = self.command_tx.send(PdfCommand::FlashcardsLoadCsv {
                input_path: PathBuf::from(self.flashcard_state.csv_path.clone()),
            });
        }

        // Imposition: options carry the input PDF paths; refresh stats + preview.
        self.impose_state.options = project.impose;
        self.impose_state.needs_regeneration = true;
        let _ = self.command_tx.send(PdfCommand::ImposeCalculateStats {
            options: self.impose_state.options.clone(),
        });

        // Typesetting: settings + re-enumerate fonts + re-load the source file.
        self.typesetting_state = project.typesetting;
        self.typesetting_state.available_fonts = pdf_typeset::available_font_families();
        if let Some(path) = self.typesetting_state.source_path.clone() {
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    self.typesetting_state.source_text = text;
                    self.typesetting_state.needs_regeneration = true;
                }
                Err(e) => log::warn!("Could not reload {}: {e}", path.display()),
            }
        }
    }

    /// Borrowed view of the current settings for serialization.
    #[cfg(not(target_arch = "wasm32"))]
    fn project_ref(&self) -> crate::project::ProjectRef<'_> {
        crate::project::ProjectRef {
            flashcards: &self.flashcard_state,
            impose: &self.impose_state.options,
            typesetting: &self.typesetting_state,
        }
    }

    /// Prompt for a path and save the current project to a `.pjproj` file.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_project_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Print Junk Project", &[crate::project::PROJECT_EXTENSION])
            .set_file_name("project.pjproj")
            .save_file()
        {
            match crate::project::write_file(&path, &self.project_ref()) {
                Ok(()) => {
                    log::info!("Saved project to {}", path.display());
                    self.startup.push_recent_project(path);
                }
                Err(e) => log::error!("Failed to save project: {e}"),
            }
        }
    }

    /// Prompt for a `.pjproj` file and load it.
    #[cfg(not(target_arch = "wasm32"))]
    fn open_project_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Print Junk Project", &[crate::project::PROJECT_EXTENSION])
            .pick_file()
        {
            self.open_project_path(path);
        }
    }

    /// Load a project from a known path (used by the dialog and the recent list).
    #[cfg(not(target_arch = "wasm32"))]
    fn open_project_path(&mut self, path: PathBuf) {
        match crate::project::read_file(&path) {
            Ok(project) => {
                self.apply_project(project);
                self.startup.push_recent_project(path);
                log::info!("Project loaded");
            }
            Err(e) => log::error!("Failed to open project: {e}"),
        }
    }
}

impl eframe::App for PrintJunkApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // egui 0.34 hands us a `Ui` instead of a `Context`. Most of the loop below
        // still works against a `Context` (input, textures, floating windows), so
        // take a cheap clone; panels are shown with `show_inside(ui, …)`.
        let ctx = ui.ctx().clone();

        // Handle drag-and-drop routed by current mode
        let dropped: Vec<_> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for path in dropped {
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            match self.mode {
                Mode::Impose
                    if ext == "pdf" && !self.impose_state.options.input_files.contains(&path) =>
                {
                    log::info!("Adding PDF to impose inputs: {}", path.display());
                    self.impose_state.options.input_files.push(path);
                    self.impose_state.needs_regeneration = true;
                }
                Mode::Flashcards if ext == "csv" => {
                    log::info!("Loading CSV: {}", path.display());
                    self.flashcard_state.csv_path = path.display().to_string();
                    let _ = self
                        .command_tx
                        .send(PdfCommand::FlashcardsLoadCsv { input_path: path });
                }
                _ if ext == "pdf" => {
                    log::info!("Loading PDF: {}", path.display());
                    let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                }
                _ => {}
            }
        }

        // Global keyboard shortcuts: Cmd+O (open), Cmd+S (save)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let cmd_o = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O));
            let cmd_s = ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S));

            if cmd_o {
                match self.mode {
                    Mode::Viewer => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .pick_file()
                        {
                            log::info!("Loading PDF: {}", path.display());
                            let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                        }
                    }
                    Mode::Impose => {
                        if let Some(paths) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .pick_files()
                        {
                            for path in paths {
                                if !self.impose_state.options.input_files.contains(&path) {
                                    self.impose_state.options.input_files.push(path);
                                    self.impose_state.needs_regeneration = true;
                                }
                            }
                        }
                    }
                    Mode::Flashcards => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .pick_file()
                        {
                            self.flashcard_state.csv_path = path.display().to_string();
                            log::info!("Loading CSV: {}", path.display());
                            let _ = self
                                .command_tx
                                .send(PdfCommand::FlashcardsLoadCsv { input_path: path });
                        }
                    }
                    Mode::Typesetting => self.typesetting_state.open_file_dialog(),
                }
            }

            if cmd_s {
                match self.mode {
                    Mode::Impose if !self.impose_state.options.input_files.is_empty() => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .set_file_name("imposed.pdf")
                            .save_file()
                        {
                            log::info!("Saving imposed PDF to: {}", path.display());
                            let _ = self.command_tx.send(PdfCommand::ImposeGenerate {
                                options: self.impose_state.options.clone(),
                                output_path: path,
                            });
                        }
                    }
                    Mode::Flashcards if !self.flashcard_state.cards.is_empty() => {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF", &["pdf"])
                            .set_file_name("flashcards.pdf")
                            .save_file()
                        {
                            log::info!("Saving flashcards to: {}", path.display());
                            let options = self.flashcard_state.to_options();
                            let _ = self.command_tx.send(PdfCommand::FlashcardsGenerate {
                                cards: self.flashcard_state.cards.clone(),
                                options,
                                output_path: path,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

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
                }
                PdfUpdate::FlashcardsLoaded { cards } => {
                    log::info!("Loaded {} flashcards from CSV", cards.len());
                    self.progress = None;
                    self.flashcard_state.cards = cards;
                    self.flashcard_state.needs_regeneration = true;
                }
                PdfUpdate::FlashcardsComplete { path, card_count } => {
                    log::info!("Generated {} flashcards → {}", card_count, path.display());
                    self.progress = None;

                    // Load preview if it's a temp file
                    if path.starts_with(std::env::temp_dir()) {
                        let _ = self.command_tx.send(PdfCommand::ViewerLoad { path });
                    }
                }
                PdfUpdate::FlashcardsConfigLoaded { options, cards } => {
                    self.flashcard_state.apply_options(&options);
                    if let Some(cards) = cards {
                        log::info!("Loaded project with {} cards", cards.len());
                        self.flashcard_state.cards = cards;
                        self.flashcard_state.csv_path.clear();
                        self.flashcard_state.paste_text.clear();
                    } else {
                        log::info!("Applied layout template");
                    }
                    self.flashcard_state.needs_regeneration = true;
                    self.progress = None;
                }
                PdfUpdate::ImposeLoaded { doc_id, page_count } => {
                    log::info!("Loaded PDF with {page_count} pages (ID: {doc_id:?})");
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
                PdfUpdate::ImposePreviewGenerated {
                    pdf_bytes,
                    page_count,
                    signatures_shown,
                    total_signatures,
                } => {
                    log::info!(
                        "Preview generated with {page_count} pages ({signatures_shown} of {total_signatures} signatures)"
                    );
                    self.impose_state.preview_page_count = page_count;
                    self.impose_state.preview_signatures_shown = Some(signatures_shown);
                    self.impose_state.preview_total_signatures = Some(total_signatures);
                    self.progress = None;

                    // Load the preview bytes into the viewer (no disk round-trip)
                    let _ = self.command_tx.send(PdfCommand::ViewerLoadBytes {
                        pdf_bytes,
                        page_count,
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
                #[cfg(not(target_arch = "wasm32"))]
                PdfUpdate::TypesetPreviewGenerated {
                    pdf_bytes,
                    page_count,
                } => {
                    self.typesetting_state.preview_page_count = page_count;
                    self.progress = None;
                    // Reuse the shared viewer pipeline (preserves scroll/page/zoom).
                    let _ = self.command_tx.send(PdfCommand::ViewerLoadBytes {
                        pdf_bytes,
                        page_count,
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                PdfUpdate::TypesetImported {
                    pdf_bytes,
                    page_count,
                    source,
                    html,
                    raw_assets,
                    body,
                    assets,
                    outline,
                    title,
                    stats,
                    asset_report,
                } => {
                    self.typesetting_state.importing = false;
                    self.typesetting_state.import_error = None;
                    self.typesetting_state.preview_page_count = page_count;
                    // Mirror the worker's title defaulting (the preview was compiled
                    // with it), so the Document section shows the effective title and
                    // later recompiles agree with what's on screen.
                    if self.typesetting_state.config.doc_title.trim().is_empty()
                        && let Some(t) = &title
                    {
                        self.typesetting_state.config.doc_title.clone_from(t);
                    }
                    // Cache the raw payload (persisted) plus the converted artifact
                    // (in-memory) for cheap recompiles on settings changes.
                    // Re-importing the same source (the assets-retry path) keeps
                    // its section overrides — the stable ids still match.
                    let overrides = self
                        .typesetting_state
                        .import
                        .take()
                        .filter(|s| s.source == source)
                        .map(|s| s.overrides)
                        .unwrap_or_default();
                    self.typesetting_state.import = Some(crate::views::ImportSession {
                        source,
                        html: (*html).clone(),
                        raw_assets: (*raw_assets).clone(),
                        overrides,
                        asset_report: Some(asset_report),
                        converted: Some(crate::views::ConvertedImport {
                            body,
                            assets,
                            outline,
                            title,
                            stats,
                        }),
                        reconvert_requested: true,
                    });
                    self.progress = None;
                    let _ = self.command_tx.send(PdfCommand::ViewerLoadBytes {
                        pdf_bytes,
                        page_count,
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                PdfUpdate::TypesetReconverted {
                    pdf_bytes,
                    page_count,
                    body,
                    assets,
                    outline,
                    title,
                    stats,
                } => {
                    self.typesetting_state.preview_page_count = page_count;
                    if let Some(import) = self.typesetting_state.import.as_mut() {
                        import.converted = Some(crate::views::ConvertedImport {
                            body,
                            assets,
                            outline,
                            title,
                            stats,
                        });
                    }
                    self.progress = None;
                    let _ = self.command_tx.send(PdfCommand::ViewerLoadBytes {
                        pdf_bytes,
                        page_count,
                    });
                }
                #[cfg(not(target_arch = "wasm32"))]
                PdfUpdate::TypesetComplete { path } => {
                    log::info!("Typeset PDF → {}", path.display());
                    self.progress = None;
                }
                #[cfg(not(target_arch = "wasm32"))]
                PdfUpdate::TypesetReadyForImpose { path } => {
                    log::info!("Sending typeset PDF to Impose: {}", path.display());
                    if !self.impose_state.options.input_files.contains(&path) {
                        self.impose_state.options.input_files.push(path);
                        self.impose_state.needs_regeneration = true;
                    }
                    self.mode = Mode::Impose;
                    self.progress = None;
                }
                PdfUpdate::Error { message } => {
                    log::error!("Error: {message}");
                    // Surface an error during an import attempt next to the field.
                    #[cfg(not(target_arch = "wasm32"))]
                    if self.typesetting_state.importing {
                        self.typesetting_state.importing = false;
                        self.typesetting_state.import_error = Some(message.clone());
                    }
                    self.progress = None;
                }
                PdfUpdate::ViewerLoaded { doc_id, page_count } => {
                    let is_standalone_viewer = matches!(self.mode, Mode::Viewer);
                    // Cloned up front so we can hand it to a new ViewerState without
                    // a second borrow of `self` while `viewer_ref` is held.
                    let command_tx = self.command_tx.clone();

                    // Get the relevant viewer for the active mode
                    let viewer_ref = match self.mode {
                        Mode::Flashcards => &mut self.flashcard_state.preview_viewer,
                        Mode::Viewer => &mut self.viewer_state,
                        Mode::Impose => &mut self.impose_state.preview_viewer,
                        #[cfg(not(target_arch = "wasm32"))]
                        Mode::Typesetting => &mut self.typesetting_state.preview_viewer,
                        // Typesetting can't run on wasm, so it never loads a preview;
                        // map to the standalone viewer to keep the match exhaustive.
                        #[cfg(target_arch = "wasm32")]
                        Mode::Typesetting => &mut self.viewer_state,
                    };

                    // Point the viewer at the new document, preserving page position
                    // in place where one already exists. The first render is driven
                    // by the DocView widget itself (its fire-and-forget request), so
                    // there's nothing to kick off here.
                    let old_doc_id = if let Some(existing) = viewer_ref {
                        Some(existing.update_for_new_document(doc_id, page_count))
                    } else {
                        *viewer_ref = Some(ViewerState::new(
                            doc_id,
                            page_count,
                            is_standalone_viewer,
                            command_tx,
                        ));
                        None
                    };

                    // Free the replaced document from worker memory.
                    if let Some(old_id) = old_doc_id {
                        let _ = self
                            .command_tx
                            .send(PdfCommand::ViewerClose { doc_id: old_id });
                    }

                    log::info!("Loaded PDF with {page_count} pages");
                    self.progress = None;
                }
                PdfUpdate::ViewerPageRendered {
                    doc_id,
                    page_index,
                    rgba_data,
                    width,
                    height,
                    zoom_level,
                    page_width_pts,
                    page_height_pts,
                } => {
                    // Hand the decoded page to whichever viewer owns this document
                    // (main viewer or an embedded preview). DocView builds the
                    // texture itself, so we only store the bitmap. At most one
                    // viewer matches a given doc_id.
                    let size_pts = (page_width_pts, page_height_pts);
                    if let Some(state) = &mut self.viewer_state
                        && state.current_doc_id() == doc_id
                    {
                        state.set_rendered_page(
                            page_index,
                            width,
                            height,
                            rgba_data.clone(),
                            size_pts,
                        );
                    }
                    for preview in self.preview_viewers_mut() {
                        if let Some(state) = preview
                            && state.current_doc_id() == doc_id
                        {
                            state.set_rendered_page(
                                page_index,
                                width,
                                height,
                                rgba_data.clone(),
                                size_pts,
                            );
                        }
                    }

                    // Prefetch adjacent pages for faster navigation
                    let total_pages = self
                        .viewer_state
                        .as_ref()
                        .map(ViewerState::total_pages)
                        .or_else(|| {
                            self.flashcard_state
                                .preview_viewer
                                .as_ref()
                                .map(ViewerState::total_pages)
                        })
                        .or_else(|| {
                            self.impose_state
                                .preview_viewer
                                .as_ref()
                                .map(ViewerState::total_pages)
                        })
                        .unwrap_or(0);

                    let mut prefetch_pages = Vec::new();
                    if page_index > 0 {
                        prefetch_pages.push(page_index - 1);
                    }
                    if page_index + 1 < total_pages {
                        prefetch_pages.push(page_index + 1);
                    }
                    // Also prefetch 2 pages ahead for smoother forward navigation
                    if page_index + 2 < total_pages {
                        prefetch_pages.push(page_index + 2);
                    }

                    if !prefetch_pages.is_empty() {
                        let _ = self.command_tx.send(PdfCommand::ViewerPrefetchPages {
                            doc_id,
                            page_indices: prefetch_pages,
                            zoom_level,
                        });
                    }

                    self.progress = None;
                }
                PdfUpdate::ViewerClosed { doc_id } => {
                    // Only clear if the closed doc_id matches the current one
                    // (stale close events arrive when old documents are replaced)
                    if let Some(state) = &self.viewer_state
                        && state.current_doc_id() == doc_id
                    {
                        self.viewer_state = None;
                        log::info!("Closed PDF");
                    }
                    for preview in self.preview_viewers_mut() {
                        if preview
                            .as_ref()
                            .is_some_and(|s| s.current_doc_id() == doc_id)
                        {
                            *preview = None;
                        }
                    }
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        let mut pending_open: Option<PathBuf> = None;

        egui::Panel::top("menu").show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.menu_button("☰", |ui| {
                    if ui.button("Show Startup Selector").clicked() {
                        self.show_startup = true;
                        ui.close();
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        ui.separator();
                        if ui.button("💾 Save Project…").clicked() {
                            self.save_project_dialog();
                            ui.close();
                        }
                        if ui.button("📂 Open Project…").clicked() {
                            self.open_project_dialog();
                            ui.close();
                        }
                        ui.menu_button("Recent Projects", |ui| {
                            if self.startup.recent_projects.is_empty() {
                                ui.label("(none)");
                            }
                            for path in &self.startup.recent_projects {
                                let label = path.file_name().map_or_else(
                                    || path.display().to_string(),
                                    |n| n.to_string_lossy().into_owned(),
                                );
                                if ui.button(label).clicked() {
                                    pending_open = Some(path.clone());
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                ui.separator();
                crate::views::tab_bar::show_tabs(ui, &mut self.mode);
            });
        });

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = pending_open {
            self.open_project_path(path);
        }

        // Status bar at bottom
        egui::Panel::bottom("status").show_inside(ui, |ui| {
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
                } else if let Some(latest) = self.logger.latest_message()
                    && ui.link(&latest).clicked()
                {
                    self.log_viewer_open = true;
                }
            });
        });

        // Log viewer window
        egui::Window::new("Log Viewer")
            .open(&mut self.log_viewer_open)
            .default_size([800.0, 400.0])
            .show(&ctx, |ui| {
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

        egui::CentralPanel::default().show_inside(ui, |ui| match self.mode {
            Mode::Viewer => show_viewer(ui, &mut self.viewer_state, &self.command_tx),
            Mode::Flashcards => show_flashcards(ui, &mut self.flashcard_state, &self.command_tx),
            Mode::Impose => show_impose(ui, &mut self.impose_state, &self.command_tx),
            Mode::Typesetting => {
                #[cfg(not(target_arch = "wasm32"))]
                show_typesetting(ui, &mut self.typesetting_state, &self.command_tx);
                #[cfg(target_arch = "wasm32")]
                ui.centered_and_justified(|ui| {
                    ui.label("Typesetting is only available in the desktop app.");
                });
            }
        });

        if self.show_startup {
            crate::startup::show_startup_modal(
                &ctx,
                &mut self.mode,
                &mut self.startup,
                &mut self.show_startup,
            );
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.startup.last_mode = self.mode;
        eframe::set_value(storage, STARTUP_KEY, &self.startup);
        // Auto-persist all modes' settings (and file paths, not contents).
        #[cfg(not(target_arch = "wasm32"))]
        eframe::set_value(storage, PROJECT_KEY, &self.project_ref());
    }
}
