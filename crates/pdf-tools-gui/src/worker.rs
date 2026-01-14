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
        process_command(
            cmd,
            &mut impose_doc_store,
            #[cfg(feature = "pdf-viewer")]
            &mut viewer_state,
            &mut command_rx,
            &update_tx,
        )
        .await;
    }
}

async fn process_command(
    cmd: PdfCommand,
    impose_doc_store: &mut handlers::impose::ImposeDocStore,
    #[cfg(feature = "pdf-viewer")] viewer_state: &mut Option<viewer::ViewerState>,
    command_rx: &mut mpsc::UnboundedReceiver<PdfCommand>,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match cmd {
        PdfCommand::FlashcardsLoadCsv { input_path } => {
            handlers::flashcards::handle_load_csv(input_path, update_tx).await;
        }
        PdfCommand::FlashcardsGenerate {
            cards,
            options,
            output_path,
        } => {
            handlers::flashcards::handle_generate(cards, options, output_path, update_tx).await;
        }
        PdfCommand::ImposeLoad { input_path } => {
            handlers::impose::handle_load(input_path, update_tx).await;
        }
        PdfCommand::ImposeProcess { .. } => {
            handlers::impose::handle_process(update_tx).await;
        }
        PdfCommand::ImposeGeneratePreview { mut options } => {
            // Drain any queued preview commands, keeping only the most recent
            while let Ok(next_cmd) = command_rx.try_recv() {
                if let PdfCommand::ImposeGeneratePreview {
                    options: new_options,
                } = next_cmd
                {
                    log::debug!("Discarding queued preview generation, using newer request");
                    options = new_options;
                } else {
                    // Non-preview command found, need to process it next
                    // Since we can't put it back, process it now before the preview
                    Box::pin(process_command(
                        next_cmd,
                        impose_doc_store,
                        #[cfg(feature = "pdf-viewer")]
                        viewer_state,
                        command_rx,
                        update_tx,
                    ))
                    .await;
                }
            }

            // Process the most recent preview
            handlers::impose::handle_generate_preview(options, impose_doc_store, update_tx).await;
        }
        PdfCommand::ImposeGenerate {
            options,
            output_path,
        } => {
            handlers::impose::handle_generate(options, output_path, update_tx).await;
        }
        PdfCommand::ImposeLoadConfig { path } => {
            handlers::impose::handle_load_config(path, update_tx).await;
        }
        PdfCommand::ImposeCalculateStats { options } => {
            handlers::impose::handle_calculate_stats(options, update_tx).await;
        }
        #[cfg(feature = "pdf-viewer")]
        PdfCommand::ViewerLoad { path } => {
            if let Some(state) = viewer_state {
                handlers::viewer::handle_load(path, state, update_tx).await;
            } else {
                let _ = update_tx.send(PdfUpdate::Error {
                    message: "PDF viewer not initialized".to_string(),
                });
            }
        }
        #[cfg(feature = "pdf-viewer")]
        PdfCommand::ViewerRenderPage {
            mut doc_id,
            mut page_index,
        } => {
            // Deduplicate render commands - keep the most recent one
            while let Ok(next_cmd) = command_rx.try_recv() {
                if let PdfCommand::ViewerRenderPage {
                    doc_id: new_doc_id,
                    page_index: new_page_index,
                } = next_cmd
                {
                    log::debug!("Discarding queued page render, using newer request");
                    doc_id = new_doc_id;
                    page_index = new_page_index;
                } else if let PdfCommand::ViewerPrefetchPages { .. } = next_cmd {
                    // Discard prefetch commands when we have a direct render pending
                    log::debug!("Discarding prefetch during page navigation");
                } else {
                    // Non-render command found, process it after rendering
                    Box::pin(process_command(
                        next_cmd,
                        impose_doc_store,
                        viewer_state,
                        command_rx,
                        update_tx,
                    ))
                    .await;
                }
            }

            if let Some(state) = viewer_state {
                handlers::viewer::handle_render_page(doc_id, page_index, state, update_tx).await;
            } else {
                let _ = update_tx.send(PdfUpdate::Error {
                    message: "PDF viewer not initialized".to_string(),
                });
            }
        }
        #[cfg(feature = "pdf-viewer")]
        PdfCommand::ViewerPrefetchPages {
            doc_id,
            page_indices,
        } => {
            if let Some(state) = viewer_state {
                handlers::viewer::handle_prefetch_pages(doc_id, page_indices, state).await;
            }
        }
        #[cfg(feature = "pdf-viewer")]
        PdfCommand::ViewerClose { doc_id } => {
            if let Some(state) = viewer_state {
                handlers::viewer::handle_close(doc_id, state, update_tx).await;
            }
        }
        #[cfg(not(feature = "pdf-viewer"))]
        PdfCommand::ViewerLoad { .. }
        | PdfCommand::ViewerRenderPage { .. }
        | PdfCommand::ViewerPrefetchPages { .. }
        | PdfCommand::ViewerClose { .. } => {
            handlers::viewer::handle_viewer_unavailable(update_tx).await;
        }
    }
}
