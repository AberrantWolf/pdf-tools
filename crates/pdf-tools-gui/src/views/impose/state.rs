use pdf_async_runtime::DocumentId;
use pdf_impose::{ImpositionOptions, ImpositionStatistics};
use std::path::PathBuf;

use super::super::ViewerState;

pub struct ImposeState {
    pub options: ImpositionOptions,
    pub preview_doc_id: Option<DocumentId>,
    pub preview_page_count: usize,
    pub stats: Option<ImpositionStatistics>,
    pub loaded_docs: Vec<(PathBuf, usize)>,
    pub preview_viewer: Option<ViewerState>,
    pub needs_regeneration: bool,
}

impl Default for ImposeState {
    fn default() -> Self {
        Self {
            options: ImpositionOptions::default(),
            preview_doc_id: None,
            preview_page_count: 0,
            stats: None,
            loaded_docs: Vec::new(),
            preview_viewer: None,
            needs_regeneration: false,
        }
    }
}
