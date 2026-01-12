use pdf_async_runtime::{PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

use crate::{handlers, viewer};

/// Async worker task that processes PDF commands and sends updates
pub async fn worker_task(
    mut command_rx: mpsc::UnboundedReceiver<PdfCommand>,
    update_tx: mpsc::UnboundedSender<PdfUpdate>,
) {
    #[cfg(feature = "pdf-viewer")]
    let mut viewer_state = match viewer::ViewerState::new() {
        Ok(state) => Some(state),
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to initialize PDF viewer: {}", e),
            });
            None
        }
    };

    let mut impose_doc_store = handlers::impose::ImposeDocStore::new();

    while let Some(cmd) = command_rx.recv().await {
        match cmd {
            PdfCommand::FlashcardsLoadCsv { input_path } => {
                handlers::flashcards::handle_load_csv(input_path, &update_tx).await;
            }
            PdfCommand::FlashcardsGenerate {
                cards,
                options,
                output_path,
            } => {
                handlers::flashcards::handle_generate(cards, options, output_path, &update_tx)
                    .await;
            }
            PdfCommand::ImposeLoad { input_path } => {
                handlers::impose::handle_load(input_path, &update_tx).await;
            }
            PdfCommand::ImposeProcess { .. } => {
                handlers::impose::handle_process(&update_tx).await;
            }
            PdfCommand::ImposeGeneratePreview { options } => {
                handlers::impose::handle_generate_preview(
                    options,
                    &mut impose_doc_store,
                    &update_tx,
                )
                .await;
            }
            PdfCommand::ImposeGenerate {
                options,
                output_path,
            } => {
                handlers::impose::handle_generate(options, output_path, &update_tx).await;
            }
            PdfCommand::ImposeLoadConfig { path } => {
                handlers::impose::handle_load_config(path, &update_tx).await;
            }
            PdfCommand::ImposeCalculateStats { options } => {
                handlers::impose::handle_calculate_stats(options, &update_tx).await;
            }
            #[cfg(feature = "pdf-viewer")]
            PdfCommand::ViewerLoad { path } => {
                if let Some(ref mut state) = viewer_state {
                    handlers::viewer::handle_load(path, state, &update_tx).await;
                } else {
                    let _ = update_tx.send(PdfUpdate::Error {
                        message: "PDF viewer not initialized".to_string(),
                    });
                }
            }
            #[cfg(feature = "pdf-viewer")]
            PdfCommand::ViewerRenderPage { doc_id, page_index } => {
                if let Some(ref mut state) = viewer_state {
                    handlers::viewer::handle_render_page(doc_id, page_index, state, &update_tx)
                        .await;
                } else {
                    let _ = update_tx.send(PdfUpdate::Error {
                        message: "PDF viewer not initialized".to_string(),
                    });
                }
            }
            #[cfg(feature = "pdf-viewer")]
            PdfCommand::ViewerClose { doc_id } => {
                if let Some(ref mut state) = viewer_state {
                    handlers::viewer::handle_close(doc_id, state, &update_tx).await;
                }
            }
            #[cfg(not(feature = "pdf-viewer"))]
            PdfCommand::ViewerLoad { .. }
            | PdfCommand::ViewerRenderPage { .. }
            | PdfCommand::ViewerClose { .. } => {
                handlers::viewer::handle_viewer_unavailable(&update_tx).await;
            }
        }
    }
}
