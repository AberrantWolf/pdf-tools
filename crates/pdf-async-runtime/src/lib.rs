use std::path::PathBuf;

// Re-export types from library crates
pub use pdf_flashcards::{Flashcard, FlashcardOptions};
pub use pdf_impose::{ImpositionLayout, ImpositionOptions};

/// Commands sent from UI to worker
#[derive(Debug)]
pub enum PdfCommand {
    FlashcardsLoadCsv {
        input_path: PathBuf,
    },
    FlashcardsGenerate {
        cards: Vec<Flashcard>,
        options: FlashcardOptions,
        output_path: PathBuf,
    },
    ImposeLoad {
        input_path: PathBuf,
    },
    ImposeProcess {
        doc_id: DocumentId,
        options: ImpositionOptions,
        output_path: PathBuf,
    },
    ViewerLoad {
        path: PathBuf,
    },
    ViewerRenderPage {
        doc_id: DocumentId,
        page_index: usize,
    },
    ViewerClose {
        doc_id: DocumentId,
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
    FlashcardsLoaded {
        cards: Vec<Flashcard>,
    },
    FlashcardsComplete {
        path: PathBuf,
        card_count: usize,
    },
    ImposeLoaded {
        doc_id: DocumentId,
        page_count: usize,
    },
    ImposeComplete {
        path: PathBuf,
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
    },
    ViewerClosed {
        doc_id: DocumentId,
    },
}

/// Handle to a loaded document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);
