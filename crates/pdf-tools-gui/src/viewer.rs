use pdf_async_runtime::DocumentId;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(feature = "pdf-viewer")]
use pdfium_render::prelude::*;

/// Initialize Pdfium, trying the vendored library first, then falling back to system
#[cfg(feature = "pdf-viewer")]
pub fn init_pdfium() -> Result<Pdfium, PdfiumError> {
    // Try to load from vendor directory (relative to workspace root)
    // When running from cargo, the working directory is the workspace root
    let vendor_path = std::env::current_dir().ok().and_then(|mut p| {
        p.push("vendor/pdfium/lib");
        if p.exists() { Some(p) } else { None }
    });

    if let Some(vendor_path) = vendor_path {
        if let Ok(binding) =
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&vendor_path))
        {
            return Ok(Pdfium::new(binding));
        }
    }

    // Fallback to system library or default search paths
    Pdfium::bind_to_system_library().map(Pdfium::new)
}

/// Cached page data
#[cfg(feature = "pdf-viewer")]
pub struct CachedPage {
    pub rgba_data: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

/// Maximum number of pages to cache
#[cfg(feature = "pdf-viewer")]
const MAX_CACHED_PAGES: usize = 50;

/// State for PDF viewer functionality
#[cfg(feature = "pdf-viewer")]
pub struct ViewerState {
    documents: HashMap<DocumentId, PathBuf>,
    page_cache: HashMap<(DocumentId, usize), CachedPage>,
    cache_order: VecDeque<(DocumentId, usize)>,
    next_doc_id: AtomicU64,
}

#[cfg(feature = "pdf-viewer")]
impl ViewerState {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            documents: HashMap::new(),
            page_cache: HashMap::new(),
            cache_order: VecDeque::new(),
            next_doc_id: AtomicU64::new(0),
        })
    }

    pub fn next_id(&self) -> DocumentId {
        DocumentId(self.next_doc_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn add_document(&mut self, doc_id: DocumentId, path: PathBuf) {
        self.documents.insert(doc_id, path);
    }

    pub fn get_document(&self, doc_id: &DocumentId) -> Option<&PathBuf> {
        self.documents.get(doc_id)
    }

    pub fn add_to_cache(&mut self, key: (DocumentId, usize), page: CachedPage) {
        // Remove if already exists (update LRU)
        if self.page_cache.contains_key(&key) {
            self.cache_order.retain(|k| k != &key);
        }

        // Evict LRU if full
        while self.cache_order.len() >= MAX_CACHED_PAGES {
            if let Some(old_key) = self.cache_order.pop_front() {
                self.page_cache.remove(&old_key);
            }
        }

        // Add to cache
        self.page_cache.insert(key, page);
        self.cache_order.push_back(key);
    }

    pub fn get_from_cache(&mut self, key: &(DocumentId, usize)) -> Option<&CachedPage> {
        if self.page_cache.contains_key(key) {
            // Update LRU order
            self.cache_order.retain(|k| k != key);
            self.cache_order.push_back(*key);
            self.page_cache.get(key)
        } else {
            None
        }
    }

    pub fn remove_document(&mut self, doc_id: DocumentId) {
        self.documents.remove(&doc_id);
        // Remove all cached pages for this document
        self.cache_order.retain(|(id, _)| *id != doc_id);
        self.page_cache.retain(|(id, _), _| *id != doc_id);
    }
}
