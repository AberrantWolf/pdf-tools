use pdf_async_runtime::PdfUpdate;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub async fn handle_load_csv(input_path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    match pdf_flashcards::load_from_csv(&input_path).await {
        Ok((cards, warnings)) => {
            for w in &warnings {
                log::warn!("Flashcard CSV: {w}");
            }
            let _ = update_tx.send(PdfUpdate::FlashcardsLoaded { cards });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load CSV: {e}"),
            });
        }
    }
}

pub async fn handle_load_config(path: PathBuf, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    use pdf_flashcards::LoadedLayout;

    match pdf_flashcards::load_flashcard_file(&path).await {
        Ok(LoadedLayout::Project(project)) => {
            log::info!(
                "Loaded flashcard project ({} cards) from {}",
                project.cards.len(),
                path.display()
            );
            let _ = update_tx.send(PdfUpdate::FlashcardsConfigLoaded {
                options: project.options,
                cards: Some(project.cards),
            });
        }
        Ok(LoadedLayout::Layout(options)) => {
            log::info!("Loaded flashcard layout from {}", path.display());
            let _ = update_tx.send(PdfUpdate::FlashcardsConfigLoaded {
                options,
                cards: None,
            });
        }
        Err(e) => {
            let _ = update_tx.send(PdfUpdate::Error {
                message: format!("Failed to load layout: {e}"),
            });
        }
    }
}

pub async fn handle_generate(
    cards: Vec<pdf_flashcards::Flashcard>,
    options: pdf_flashcards::FlashcardOptions,
    output_path: PathBuf,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
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
