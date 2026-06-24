use pdf_async_runtime::{PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

use crate::{handlers, viewer};

/// Async worker task that processes PDF commands and sends updates
pub async fn worker_task(
    mut command_rx: mpsc::UnboundedReceiver<PdfCommand>,
    update_tx: mpsc::UnboundedSender<PdfUpdate>,
) {
    #[cfg(feature = "pdf-viewer")]
    let mut viewer_state = Some(viewer::ViewerState::new());

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
        PdfCommand::FlashcardsLoadCsv {
            input_path,
            has_headers,
        } => {
            handlers::flashcards::handle_load_csv(input_path, has_headers, update_tx).await;
        }
        PdfCommand::FlashcardsGenerate {
            cards,
            options,
            output_path,
        } => {
            handlers::flashcards::handle_generate(cards, options, output_path, update_tx).await;
        }
        PdfCommand::FlashcardsLoadConfig { path } => {
            handlers::flashcards::handle_load_config(path, update_tx).await;
        }
        PdfCommand::ImposeLoad { input_path } => {
            handlers::impose::handle_load(input_path, update_tx).await;
        }
        PdfCommand::ImposeProcess { .. } => {
            handlers::impose::handle_process(update_tx);
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
        PdfCommand::ViewerLoadBytes {
            pdf_bytes,
            page_count,
        } => {
            if let Some(state) = viewer_state {
                handlers::viewer::handle_load_bytes(pdf_bytes, page_count, state, update_tx);
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
            mut zoom_level,
        } => {
            // Deduplicate render commands - keep the most recent one
            while let Ok(next_cmd) = command_rx.try_recv() {
                if let PdfCommand::ViewerRenderPage {
                    doc_id: new_doc_id,
                    page_index: new_page_index,
                    zoom_level: new_zoom_level,
                } = next_cmd
                {
                    log::debug!("Discarding queued page render, using newer request");
                    doc_id = new_doc_id;
                    page_index = new_page_index;
                    zoom_level = new_zoom_level;
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
                handlers::viewer::handle_render_page(
                    doc_id, page_index, zoom_level, state, update_tx,
                )
                .await;
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
            zoom_level,
        } => {
            if let Some(state) = viewer_state {
                handlers::viewer::handle_prefetch_pages(doc_id, page_indices, zoom_level, state)
                    .await;
            }
        }
        #[cfg(feature = "pdf-viewer")]
        PdfCommand::ViewerClose { doc_id } => {
            if let Some(state) = viewer_state {
                handlers::viewer::handle_close(doc_id, state, update_tx);
            }
        }
        #[cfg(not(feature = "pdf-viewer"))]
        PdfCommand::ViewerLoad { .. }
        | PdfCommand::ViewerLoadBytes { .. }
        | PdfCommand::ViewerRenderPage { .. }
        | PdfCommand::ViewerPrefetchPages { .. }
        | PdfCommand::ViewerClose { .. } => {
            handlers::viewer::handle_viewer_unavailable(update_tx).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetGeneratePreview {
            mut input,
            mut config,
        } => {
            // Typst compilation is slow; keep only the most recent preview request.
            while let Ok(next_cmd) = command_rx.try_recv() {
                if let PdfCommand::TypesetGeneratePreview {
                    input: new_input,
                    config: new_config,
                } = next_cmd
                {
                    log::debug!("Discarding queued typeset preview, using newer request");
                    input = new_input;
                    config = new_config;
                } else {
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
            handlers::typesetting::handle_generate_preview(input, config, update_tx).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetGenerate {
            input,
            config,
            output_path,
        } => {
            handlers::typesetting::handle_generate(input, config, output_path, update_tx).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetSendToImpose { input, config } => {
            handlers::typesetting::handle_send_to_impose(input, config, update_tx).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetImport { source, config } => {
            handlers::typesetting::handle_import(source, config, update_tx).await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetReconvert {
            html,
            raw_assets,
            overrides,
            config,
        } => {
            handlers::typesetting::handle_reconvert(html, raw_assets, overrides, config, update_tx)
                .await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetCompileImported {
            mut body,
            mut assets,
            mut outline,
            mut overrides,
            mut config,
        } => {
            // Like the text-preview path, settings changes can fire every frame;
            // keep only the most recent recompile request.
            while let Ok(next_cmd) = command_rx.try_recv() {
                if let PdfCommand::TypesetCompileImported {
                    body: new_body,
                    assets: new_assets,
                    outline: new_outline,
                    overrides: new_overrides,
                    config: new_config,
                } = next_cmd
                {
                    log::debug!("Discarding queued import recompile, using newer request");
                    body = new_body;
                    assets = new_assets;
                    outline = new_outline;
                    overrides = new_overrides;
                    config = new_config;
                } else {
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
            handlers::typesetting::handle_compile_imported(
                body, assets, outline, overrides, config, update_tx,
            )
            .await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetGenerateImported {
            body,
            assets,
            outline,
            overrides,
            config,
            output_path,
        } => {
            handlers::typesetting::handle_generate_imported(
                body,
                assets,
                outline,
                overrides,
                config,
                output_path,
                update_tx,
            )
            .await;
        }
        #[cfg(not(target_arch = "wasm32"))]
        PdfCommand::TypesetSendImportedToImpose {
            body,
            assets,
            outline,
            overrides,
            config,
        } => {
            handlers::typesetting::handle_send_imported_to_impose(
                body, assets, outline, overrides, config, update_tx,
            )
            .await;
        }
    }
}
