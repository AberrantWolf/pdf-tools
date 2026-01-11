use pdf_async_runtime::{DocumentId, PdfCommand, PdfUpdate};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

#[cfg(feature = "pdf-viewer")]
use pdfium_render::prelude::*;

/// Initialize Pdfium, trying the vendored library first, then falling back to system
#[cfg(feature = "pdf-viewer")]
fn init_pdfium() -> Result<Pdfium, PdfiumError> {
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
struct CachedPage {
    rgba_data: Vec<u8>,
    width: usize,
    height: usize,
}

/// Maximum number of pages to cache
#[cfg(feature = "pdf-viewer")]
const MAX_CACHED_PAGES: usize = 50;

/// State for PDF viewer functionality
#[cfg(feature = "pdf-viewer")]
struct ViewerState {
    documents: HashMap<DocumentId, PathBuf>,
    page_cache: HashMap<(DocumentId, usize), CachedPage>,
    cache_order: VecDeque<(DocumentId, usize)>,
    next_doc_id: AtomicU64,
}

#[cfg(feature = "pdf-viewer")]
impl ViewerState {
    fn new() -> Result<Self, String> {
        Ok(Self {
            documents: HashMap::new(),
            page_cache: HashMap::new(),
            cache_order: VecDeque::new(),
            next_doc_id: AtomicU64::new(0),
        })
    }

    fn next_id(&self) -> DocumentId {
        DocumentId(self.next_doc_id.fetch_add(1, Ordering::SeqCst))
    }

    fn add_to_cache(&mut self, key: (DocumentId, usize), page: CachedPage) {
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

    fn get_from_cache(&mut self, key: &(DocumentId, usize)) -> Option<&CachedPage> {
        if self.page_cache.contains_key(key) {
            // Update LRU order
            self.cache_order.retain(|k| k != key);
            self.cache_order.push_back(*key);
            self.page_cache.get(key)
        } else {
            None
        }
    }

    fn remove_document(&mut self, doc_id: DocumentId) {
        self.documents.remove(&doc_id);
        // Remove all cached pages for this document
        self.cache_order.retain(|(id, _)| *id != doc_id);
        self.page_cache.retain(|(id, _), _| *id != doc_id);
    }
}

/// Async worker task that processes PDF commands and sends updates
pub async fn worker_task(
    mut command_rx: mpsc::UnboundedReceiver<PdfCommand>,
    update_tx: mpsc::UnboundedSender<PdfUpdate>,
) {
    #[cfg(feature = "pdf-viewer")]
    let mut viewer_state = match ViewerState::new() {
        Ok(state) => Some(state),
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to initialize PDF viewer: {}", e),
            });
            None
        }
    };
    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            PdfCommand::FlashcardsLoadCsv { input_path } => {
                match pdf_flashcards::load_from_csv(&input_path).await {
                    Ok(cards) => {
                        let _ = update_tx.send(PdfUpdate::FlashcardsLoaded { cards });
                    }
                    Err(e) => {
                        let _ = update_tx.send(PdfUpdate::Error {
                            message: format!("Failed to load CSV: {e}"),
                        });
                    }
                }
            }
            PdfCommand::FlashcardsGenerate {
                cards,
                options,
                output_path,
            } => match pdf_flashcards::generate_pdf(&cards, &options, &output_path).await {
                Ok(()) => {
                    let _ = update_tx.send(PdfUpdate::FlashcardsComplete {
                        path: output_path,
                        card_count: cards.len(),
                    });
                }
                Err(e) => {
                    let _ = update_tx.send(PdfUpdate::Error {
                        message: format!("Failed to generate PDF: {e}"),
                    });
                }
            },
            PdfCommand::ImposeLoad { input_path } => {
                match pdf_impose::load_pdf(&input_path).await {
                    Ok(doc) => {
                        let page_count = doc.get_pages().len();
                        // For now, we don't store documents - just report loaded
                        // In a full implementation, would store in a HashMap
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
            PdfCommand::ImposeProcess {
                doc_id: _,
                options: _,
                output_path: _,
            } => {
                // Simplified: load, impose, save in one step
                // In a full implementation, would retrieve from HashMap using doc_id
                let _ = update_tx.send(PdfUpdate::Error {
                    message: "Imposition not yet fully implemented".to_string(),
                });
            }
            #[cfg(feature = "pdf-viewer")]
            PdfCommand::ViewerLoad { path } => {
                if let Some(ref mut state) = viewer_state {
                    let path_clone = path.clone();

                    // Load PDF to get page count
                    match tokio::task::spawn_blocking(move || {
                        let pdfium = init_pdfium()?;
                        let document = pdfium.load_pdf_from_file(&path_clone, None)?;
                        let page_count = document.pages().len();
                        Ok::<_, PdfiumError>(page_count)
                    })
                    .await
                    {
                        Ok(Ok(page_count)) => {
                            let doc_id = state.next_id();
                            state.documents.insert(doc_id, path);
                            let _ = update_tx.send(PdfUpdate::ViewerLoaded {
                                doc_id,
                                page_count: page_count as usize,
                            });
                        }
                        Ok(Err(e)) => {
                            let _ = update_tx.send(PdfUpdate::Error {
                                message: format!("Failed to load PDF: {}", e),
                            });
                        }
                        Err(e) => {
                            let _ = update_tx.send(PdfUpdate::Error {
                                message: format!("Task join error: {}", e),
                            });
                        }
                    }
                } else {
                    let _ = update_tx.send(PdfUpdate::Error {
                        message: "PDF viewer not initialized".to_string(),
                    });
                }
            }
            #[cfg(feature = "pdf-viewer")]
            PdfCommand::ViewerRenderPage { doc_id, page_index } => {
                if let Some(ref mut state) = viewer_state {
                    let cache_key = (doc_id, page_index);

                    // Check cache first
                    if let Some(cached) = state.get_from_cache(&cache_key) {
                        let _ = update_tx.send(PdfUpdate::ViewerPageRendered {
                            doc_id,
                            page_index,
                            width: cached.width,
                            height: cached.height,
                            rgba_data: cached.rgba_data.clone(),
                        });
                    } else if let Some(pdf_path) = state.documents.get(&doc_id).cloned() {
                        // Not in cache, need to render
                        match tokio::task::spawn_blocking(move || {
                            let pdfium = init_pdfium()?;
                            let document = pdfium.load_pdf_from_file(&pdf_path, None)?;
                            let page = document.pages().get(page_index as u16)?;

                            let config = PdfRenderConfig::new()
                                .set_target_width(600)
                                .set_maximum_height(800);

                            let bitmap = page.render_with_config(&config)?;
                            let rgba_data = bitmap.as_rgba_bytes().to_vec();
                            let width = bitmap.width() as usize;
                            let height = bitmap.height() as usize;

                            Ok::<_, PdfiumError>((rgba_data, width, height))
                        })
                        .await
                        {
                            Ok(Ok((rgba_data, width, height))) => {
                                // Add to cache
                                state.add_to_cache(
                                    cache_key,
                                    CachedPage {
                                        rgba_data: rgba_data.clone(),
                                        width,
                                        height,
                                    },
                                );

                                let _ = update_tx.send(PdfUpdate::ViewerPageRendered {
                                    doc_id,
                                    page_index,
                                    width,
                                    height,
                                    rgba_data,
                                });
                            }
                            Ok(Err(e)) => {
                                let _ = update_tx.send(PdfUpdate::Error {
                                    message: format!("Failed to render page: {}", e),
                                });
                            }
                            Err(e) => {
                                let _ = update_tx.send(PdfUpdate::Error {
                                    message: format!("Task join error: {}", e),
                                });
                            }
                        }
                    } else {
                        let _ = update_tx.send(PdfUpdate::Error {
                            message: format!("Document not found: {:?}", doc_id),
                        });
                    }
                } else {
                    let _ = update_tx.send(PdfUpdate::Error {
                        message: "PDF viewer not initialized".to_string(),
                    });
                }
            }
            #[cfg(feature = "pdf-viewer")]
            PdfCommand::ViewerClose { doc_id } => {
                if let Some(ref mut state) = viewer_state {
                    state.remove_document(doc_id);
                    let _ = update_tx.send(PdfUpdate::ViewerClosed { doc_id });
                }
            }
            #[cfg(not(feature = "pdf-viewer"))]
            PdfCommand::ViewerLoad { .. }
            | PdfCommand::ViewerRenderPage { .. }
            | PdfCommand::ViewerClose { .. } => {
                let _ = update_tx.send(PdfUpdate::Error {
                    message: "PDF viewer not available (pdf-viewer feature disabled)".to_string(),
                });
            }
        }
    }
}
