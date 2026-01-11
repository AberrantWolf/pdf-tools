use pdf_async_runtime::PdfUpdate;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub async fn handle_load(input_path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
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

pub async fn handle_process(update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    // Simplified: load, impose, save in one step
    // In a full implementation, would retrieve from HashMap using doc_id
    let _ = update_tx.send(PdfUpdate::Error {
        message: "Imposition not yet fully implemented".to_string(),
    });
}
