use pdf_async_runtime::{DocumentId, PdfUpdate};
use std::path::PathBuf;
use tokio::sync::mpsc;

#[cfg(feature = "pdf-viewer")]
use crate::viewer::{CachedPage, ViewerState, init_pdfium};

#[cfg(feature = "pdf-viewer")]
use pdfium_render::prelude::*;

#[cfg(feature = "pdf-viewer")]
pub async fn handle_load(
    path: PathBuf,
    state: &mut ViewerState,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
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
            state.add_document(doc_id, path);
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
}

#[cfg(feature = "pdf-viewer")]
pub async fn handle_render_page(
    doc_id: DocumentId,
    page_index: usize,
    state: &mut ViewerState,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
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
    } else if let Some(pdf_path) = state.get_document(&doc_id).cloned() {
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
}

#[cfg(feature = "pdf-viewer")]
pub async fn handle_close(
    doc_id: DocumentId,
    state: &mut ViewerState,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    state.remove_document(doc_id);
    let _ = update_tx.send(PdfUpdate::ViewerClosed { doc_id });
}

#[cfg(not(feature = "pdf-viewer"))]
pub async fn handle_viewer_unavailable(update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    let _ = update_tx.send(PdfUpdate::Error {
        message: "PDF viewer not available (pdf-viewer feature disabled)".to_string(),
    });
}
