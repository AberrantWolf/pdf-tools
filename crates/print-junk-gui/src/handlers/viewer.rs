use pdf_async_runtime::{DocumentId, PdfUpdate};
use std::path::PathBuf;
use tokio::sync::mpsc;

#[cfg(feature = "pdf-viewer")]
use crate::viewer::{CachedPage, DocumentSource, ViewerState, quantize_zoom};

/// Render one page of a [`DocumentSource`] to RGBA bytes at `scale` (pixels per
/// point), via the shared `junk-libs-platen` core. Returns the pixels, raster
/// size, and the page's native point size. Runs inside `spawn_blocking`;
/// renders are self-contained, so concurrent calls don't serialize.
#[cfg(feature = "pdf-viewer")]
fn render_source_page(
    source: &DocumentSource,
    page_index: usize,
    scale: f32,
) -> anyhow::Result<(Vec<u8>, usize, usize, f32, f32)> {
    let (image, (width_pts, height_pts)) = match source {
        DocumentSource::File(path) => {
            junk_libs_platen::render_page_bitmap(path, page_index, scale)?
        }
        DocumentSource::Bytes(bytes) => {
            junk_libs_platen::render_page_bitmap_from_bytes(bytes, page_index, scale)?
        }
    };
    let (width, height) = (image.width() as usize, image.height() as usize);
    Ok((image.into_raw(), width, height, width_pts, height_pts))
}

/// Render scale (pixels per point) for a requested zoom fraction, treating the
/// legacy `0.0` sentinel (and any non-positive value) as 100%.
#[cfg(feature = "pdf-viewer")]
fn render_scale(zoom_level: f32) -> f32 {
    if zoom_level <= 0.0 { 1.0 } else { zoom_level }
}

#[cfg(feature = "pdf-viewer")]
pub async fn handle_load(
    path: PathBuf,
    state: &mut ViewerState,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let path_clone = path.clone();

    // Load PDF to get page count (no rendering)
    match tokio::task::spawn_blocking(move || {
        Ok::<_, anyhow::Error>(junk_libs_platen::page_count(&path_clone)?)
    })
    .await
    {
        Ok(Ok(page_count)) => {
            let doc_id = state.next_id();
            state.add_document(doc_id, path);
            let _ = update_tx.send(PdfUpdate::ViewerLoaded { doc_id, page_count });
        }
        Ok(Err(e)) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load PDF: {e}"),
            });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Task join error: {e}"),
            });
        }
    }
}

#[cfg(feature = "pdf-viewer")]
pub fn handle_load_bytes(
    pdf_bytes: Vec<u8>,
    page_count: usize,
    state: &mut ViewerState,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let doc_id = state.next_id();
    state.add_document_bytes(doc_id, pdf_bytes);
    let _ = update_tx.send(PdfUpdate::ViewerLoaded { doc_id, page_count });
}

#[cfg(feature = "pdf-viewer")]
pub async fn handle_render_page(
    doc_id: DocumentId,
    page_index: usize,
    zoom_level: f32,
    state: &mut ViewerState,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let quantized = quantize_zoom(zoom_level);
    let cache_key = (doc_id, page_index, quantized);

    // Check cache first
    if let Some(cached) = state.get_from_cache(&cache_key) {
        let _ = update_tx.send(PdfUpdate::ViewerPageRendered {
            doc_id,
            page_index,
            width: cached.width,
            height: cached.height,
            rgba_data: cached.rgba_data.clone(),
            zoom_level,
            // We don't store native size in cache, so re-render path will provide it.
            // For cache hits, the UI already has page_native_size from a prior render.
            page_width_pts: 0.0,
            page_height_pts: 0.0,
        });
    } else if let Some(source) = state.get_document(doc_id).cloned() {
        // Not in cache, need to render
        let scale = render_scale(zoom_level);
        match tokio::task::spawn_blocking(move || render_source_page(&source, page_index, scale))
            .await
        {
            Ok(Ok((rgba_data, width, height, page_width_pts, page_height_pts))) => {
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
                    zoom_level,
                    page_width_pts,
                    page_height_pts,
                });
            }
            Ok(Err(e)) => {
                let _ = update_tx.send(PdfUpdate::Error {
                    message: format!("Failed to render page: {e}"),
                });
            }
            Err(e) => {
                let _ = update_tx.send(PdfUpdate::Error {
                    message: format!("Task join error: {e}"),
                });
            }
        }
    } else {
        let _ = update_tx.send(PdfUpdate::Error {
            message: format!("Document not found: {doc_id:?}"),
        });
    }
}

/// Prefetch pages into cache without sending updates to UI
/// This runs silently in the background to warm the cache
#[cfg(feature = "pdf-viewer")]
pub async fn handle_prefetch_pages(
    doc_id: DocumentId,
    page_indices: Vec<usize>,
    zoom_level: f32,
    state: &mut ViewerState,
) {
    let quantized = quantize_zoom(zoom_level);

    for page_index in page_indices {
        let cache_key = (doc_id, page_index, quantized);

        // Skip if already cached
        if state.get_from_cache(&cache_key).is_some() {
            continue;
        }

        if let Some(source) = state.get_document(doc_id).cloned() {
            // Render to cache silently (no UI update)
            let scale = render_scale(zoom_level);
            match tokio::task::spawn_blocking(move || render_source_page(&source, page_index, scale))
                .await
            {
                Ok(Ok((rgba_data, width, height, _, _))) => {
                    state.add_to_cache(
                        cache_key,
                        CachedPage {
                            rgba_data,
                            width,
                            height,
                        },
                    );
                    log::debug!("Prefetched page {page_index} into cache");
                }
                Ok(Err(e)) => {
                    log::warn!("Failed to prefetch page {page_index}: {e}");
                }
                Err(e) => {
                    log::warn!("Prefetch task join error for page {page_index}: {e}");
                }
            }
        }
    }
}

#[cfg(feature = "pdf-viewer")]
pub fn handle_close(
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
