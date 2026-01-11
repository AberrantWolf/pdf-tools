use crate::types::{Flashcard, Result};
use std::path::Path;

pub async fn load_from_csv(path: impl AsRef<Path>) -> Result<Vec<Flashcard>> {
    let path = path.as_ref().to_owned();

    let contents = tokio::fs::read_to_string(&path).await?;

    let cards = tokio::task::spawn_blocking(move || {
        let mut reader = csv::Reader::from_reader(contents.as_bytes());
        let mut cards = Vec::new();

        for result in reader.records() {
            let record = result?;
            if record.len() >= 2 {
                cards.push(Flashcard {
                    front: record[0].to_string(),
                    back: record[1].to_string(),
                });
            }
        }
        Ok::<_, crate::types::FlashcardError>(cards)
    })
    .await??;

    Ok(cards)
}
