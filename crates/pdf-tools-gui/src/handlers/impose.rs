use lopdf::Document;
use pdf_async_runtime::{ImpositionOptions, ImpositionStatistics, PdfUpdate};
use pdf_impose::{calculate_statistics, generate_preview, impose, load_multiple_pdfs, save_pdf};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

// Store loaded documents for impose operations
static NEXT_DOC_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub struct ImposeDocStore {
    documents: HashMap<u64, Document>,
}

impl ImposeDocStore {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    pub fn store(&mut self, doc: Document) -> u64 {
        let id = NEXT_DOC_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.documents.insert(id, doc);
        id
    }

    pub fn get(&self, id: u64) -> Option<&Document> {
        self.documents.get(&id)
    }

    #[allow(dead_code)]
    pub fn remove(&mut self, id: u64) -> Option<Document> {
        self.documents.remove(&id)
    }
}

pub async fn handle_load(input_path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    match pdf_impose::load_pdf(&input_path).await {
        Ok(doc) => {
            let page_count = doc.get_pages().len();
            let _ = update_tx.send(PdfUpdate::ImposeLoaded {
                doc_id: pdf_async_runtime::DocumentId(0),
                page_count,
            });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDF: {e}"),
            });
        }
    }
}

pub async fn handle_process(update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    let _ = update_tx.send(PdfUpdate::Error {
        message: "Imposition not yet fully implemented".to_string(),
    });
}

pub async fn handle_generate_preview(
    options: ImpositionOptions,
    doc_store: &mut ImposeDocStore,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    if options.input_files.is_empty() {
        let _ = update_tx.send(PdfUpdate::Error {
            message: "No input files specified".to_string(),
        });
        return;
    }

    // Load documents
    let paths: Vec<PathBuf> = options.input_files.iter().cloned().collect();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs: {}", e),
            });
            return;
        }
    };

    // Generate preview (first signature or reasonable sample)
    let preview = match generate_preview(&documents, &options, 4).await {
        Ok(doc) => doc,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to generate preview: {}", e),
            });
            return;
        }
    };

    let page_count = preview.get_pages().len();
    let doc_id = doc_store.store(preview);

    let _ = update_tx.send(PdfUpdate::ImposePreviewGenerated {
        doc_id: pdf_async_runtime::DocumentId(doc_id),
        page_count,
    });
}

pub async fn handle_generate(
    options: ImpositionOptions,
    output_path: PathBuf,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    if options.input_files.is_empty() {
        let _ = update_tx.send(PdfUpdate::Error {
            message: "No input files specified".to_string(),
        });
        return;
    }

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Loading PDFs".to_string(),
        current: 0,
        total: options.input_files.len(),
    });

    // Load documents
    let paths: Vec<PathBuf> = options.input_files.iter().cloned().collect();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs: {}", e),
            });
            return;
        }
    };

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Imposing pages".to_string(),
        current: 1,
        total: 3,
    });

    // Impose
    let imposed = match impose(&documents, &options).await {
        Ok(doc) => doc,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to impose PDF: {}", e),
            });
            return;
        }
    };

    let _ = update_tx.send(PdfUpdate::Progress {
        operation: "Saving PDF".to_string(),
        current: 2,
        total: 3,
    });

    // Save
    if let Err(e) = save_pdf(imposed, &output_path).await {
        let _ = update_tx.send(PdfUpdate::Error {
            message: format!("Failed to save PDF: {}", e),
        });
        return;
    }

    let _ = update_tx.send(PdfUpdate::ImposeComplete { path: output_path });
}

pub async fn handle_load_config(path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    match ImpositionOptions::load(&path).await {
        Ok(options) => {
            let _ = update_tx.send(PdfUpdate::ImposeConfigLoaded { options });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load configuration: {}", e),
            });
        }
    }
}

pub async fn handle_calculate_stats(
    options: ImpositionOptions,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    if options.input_files.is_empty() {
        return;
    }

    // Load documents
    let paths: Vec<PathBuf> = options.input_files.iter().cloned().collect();
    let documents = match load_multiple_pdfs(&paths).await {
        Ok(docs) => docs,
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDFs for stats: {}", e),
            });
            return;
        }
    };

    // Calculate statistics
    match calculate_statistics(&documents, &options) {
        Ok(stats) => {
            let _ = update_tx.send(PdfUpdate::ImposeStatsCalculated { stats });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to calculate statistics: {}", e),
            });
        }
    }
}
