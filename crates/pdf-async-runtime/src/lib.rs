use std::path::PathBuf;

// Re-export types from library crates
pub use pdf_flashcards::{ColumnRole, CsvTable, Flashcard, FlashcardOptions, FlashcardWarning};
#[cfg(not(target_arch = "wasm32"))]
pub use pdf_import::{ArchiveStatus, AssetReport};
pub use pdf_impose::{ImpositionOptions, ImpositionStatistics};
#[cfg(not(target_arch = "wasm32"))]
pub use pdf_typeset::{
    FRONT_MATTER_ID, ImportStats, InputFormat, OutlineEntry, SectionOverride, TypesetConfig,
    TypesetInput,
};

/// Named, in-memory document assets (math SVGs, fetched images) shared across the
/// UI/worker boundary. `Arc` keeps the per-settings-change recompile messages
/// cheap pointer clones rather than re-copying image bytes.
#[cfg(not(target_arch = "wasm32"))]
pub type SharedAssets = std::sync::Arc<Vec<(String, Vec<u8>)>>;

/// An imported document's heading outline, shared like [`SharedAssets`].
#[cfg(not(target_arch = "wasm32"))]
pub type SharedOutline = std::sync::Arc<Vec<OutlineEntry>>;

/// Per-section overrides keyed by [`OutlineEntry::id`] (small; sent by value).
#[cfg(not(target_arch = "wasm32"))]
pub type SectionOverrides = std::collections::HashMap<String, SectionOverride>;

/// Commands sent from UI to worker
#[derive(Debug)]
pub enum PdfCommand {
    FlashcardsLoadCsv {
        input_path: PathBuf,
        /// Whether the first row names the columns (vs. being data).
        has_headers: bool,
    },
    FlashcardsGenerate {
        cards: Vec<Flashcard>,
        options: FlashcardOptions,
        output_path: PathBuf,
    },
    /// Load a saved flashcard layout or project (auto-detects which).
    FlashcardsLoadConfig {
        path: PathBuf,
    },
    ImposeLoad {
        input_path: PathBuf,
    },
    ImposeProcess {
        doc_id: DocumentId,
        options: ImpositionOptions,
        output_path: PathBuf,
    },
    ImposeGeneratePreview {
        options: ImpositionOptions,
    },
    ImposeGenerate {
        options: ImpositionOptions,
        output_path: PathBuf,
    },
    ImposeLoadConfig {
        path: PathBuf,
    },
    ImposeCalculateStats {
        options: ImpositionOptions,
    },
    ViewerLoad {
        path: PathBuf,
    },
    ViewerLoadBytes {
        pdf_bytes: Vec<u8>,
        page_count: usize,
    },
    ViewerRenderPage {
        doc_id: DocumentId,
        page_index: usize,
        /// Zoom level as a fraction (1.0 = 100%). Use 0.0 for legacy fixed-size rendering.
        zoom_level: f32,
    },
    /// Prefetch pages for faster navigation (lower priority than direct renders)
    ViewerPrefetchPages {
        doc_id: DocumentId,
        page_indices: Vec<usize>,
        /// Zoom level as a fraction (1.0 = 100%). Use 0.0 for legacy fixed-size rendering.
        zoom_level: f32,
    },
    ViewerClose {
        doc_id: DocumentId,
    },
    /// Typeset the input and return a preview PDF (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetGeneratePreview {
        input: TypesetInput,
        config: TypesetConfig,
    },
    /// Typeset the input and write the PDF to `output_path` (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetGenerate {
        input: TypesetInput,
        config: TypesetConfig,
        output_path: PathBuf,
    },
    /// Typeset the input to a temp PDF and hand it off to the imposition mode.
    #[cfg(not(target_arch = "wasm32"))]
    TypesetSendToImpose {
        input: TypesetInput,
        config: TypesetConfig,
    },
    /// Acquire a document (URL / arXiv id / local file), convert it, and return a
    /// preview plus the raw + converted artifacts for caching (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetImport {
        source: String,
        config: TypesetConfig,
    },
    /// Re-convert a previously-imported document from its cached raw HTML and
    /// assets (offline, on restore), then compile a preview (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetReconvert {
        html: std::sync::Arc<String>,
        raw_assets: SharedAssets,
        overrides: SectionOverrides,
        config: TypesetConfig,
    },
    /// Recompile an already-converted import to a preview — the cheap path for
    /// settings changes (no network, no re-conversion) (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetCompileImported {
        body: std::sync::Arc<String>,
        assets: SharedAssets,
        outline: SharedOutline,
        overrides: SectionOverrides,
        config: TypesetConfig,
    },
    /// Compile a converted import and write the PDF to `output_path` (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetGenerateImported {
        body: std::sync::Arc<String>,
        assets: SharedAssets,
        outline: SharedOutline,
        overrides: SectionOverrides,
        config: TypesetConfig,
        output_path: PathBuf,
    },
    /// Compile a converted import to a temp PDF and hand it to imposition (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetSendImportedToImpose {
        body: std::sync::Arc<String>,
        assets: SharedAssets,
        outline: SharedOutline,
        overrides: SectionOverrides,
        config: TypesetConfig,
    },
}

/// Updates sent from worker to UI
#[derive(Debug, Clone)]
pub enum PdfUpdate {
    Progress {
        operation: String,
        current: usize,
        total: usize,
    },
    /// A CSV/paste was parsed into a column table; the UI maps columns to card
    /// sides (see [`CsvTable::to_cards`]).
    FlashcardsTableLoaded {
        table: CsvTable,
        warnings: Vec<FlashcardWarning>,
    },
    FlashcardsComplete {
        path: PathBuf,
        card_count: usize,
    },
    /// A saved layout or project was loaded. `cards` is `Some` for a project
    /// (replace the loaded set) and `None` for a layout-only template (keep
    /// any existing cards).
    FlashcardsConfigLoaded {
        options: FlashcardOptions,
        cards: Option<Vec<Flashcard>>,
    },
    ImposeLoaded {
        doc_id: DocumentId,
        page_count: usize,
    },
    ImposeComplete {
        path: PathBuf,
    },
    ImposePreviewGenerated {
        pdf_bytes: Vec<u8>,
        page_count: usize,
        /// Number of signatures included in this preview
        signatures_shown: usize,
        /// Total signatures in the full imposition
        total_signatures: usize,
    },
    ImposeConfigLoaded {
        options: ImpositionOptions,
    },
    ImposeStatsCalculated {
        stats: ImpositionStatistics,
    },
    Error {
        message: String,
    },
    ViewerLoaded {
        doc_id: DocumentId,
        page_count: usize,
    },
    ViewerPageRendered {
        doc_id: DocumentId,
        page_index: usize,
        width: usize,
        height: usize,
        rgba_data: Vec<u8>,
        /// The zoom level this was rendered at (fraction, 1.0 = 100%)
        zoom_level: f32,
        /// Native page width in PDF points
        page_width_pts: f32,
        /// Native page height in PDF points
        page_height_pts: f32,
    },
    ViewerClosed {
        doc_id: DocumentId,
    },
    /// A typeset preview PDF is ready (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetPreviewGenerated {
        pdf_bytes: Vec<u8>,
        page_count: usize,
    },
    /// A typeset PDF was written to `path` (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetComplete {
        path: PathBuf,
    },
    /// A typeset PDF at `path` is ready to be loaded into the imposition mode.
    #[cfg(not(target_arch = "wasm32"))]
    TypesetReadyForImpose {
        path: PathBuf,
    },
    /// A document import finished: a preview is ready, along with the raw payload
    /// to persist (`source`/`html`/`raw_assets`) and the converted artifact to
    /// cache in-memory for cheap recompiles (`body`/`assets`) (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetImported {
        pdf_bytes: Vec<u8>,
        page_count: usize,
        source: String,
        html: std::sync::Arc<String>,
        raw_assets: SharedAssets,
        body: std::sync::Arc<String>,
        assets: SharedAssets,
        outline: SharedOutline,
        title: Option<String>,
        stats: ImportStats,
        /// What the asset pipeline did (source archive, figure upgrades).
        asset_report: AssetReport,
    },
    /// A cached import was re-converted on restore: a preview is ready and the
    /// freshly-converted artifact should replace the in-memory cache (desktop-only).
    #[cfg(not(target_arch = "wasm32"))]
    TypesetReconverted {
        pdf_bytes: Vec<u8>,
        page_count: usize,
        body: std::sync::Arc<String>,
        assets: SharedAssets,
        outline: SharedOutline,
        title: Option<String>,
        stats: ImportStats,
    },
}

/// Handle to a loaded document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);
