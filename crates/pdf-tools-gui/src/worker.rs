use pdf_async_runtime::{PdfCommand, PdfUpdate};
use tokio::sync::mpsc;

/// Async worker task that processes PDF commands and sends updates
pub async fn worker_task(
    mut command_rx: mpsc::UnboundedReceiver<PdfCommand>,
    update_tx: mpsc::UnboundedSender<PdfUpdate>,
) {
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
            } => {
                match pdf_flashcards::generate_pdf(&cards, &options, &output_path).await {
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
                }
            }
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
        }
    }
}
