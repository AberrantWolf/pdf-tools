//! Saving and loading flashcard layouts.
//!
//! Two on-disk shapes are supported, both JSON:
//!
//! - A **layout** is just a [`FlashcardOptions`] — a reusable, content-independent
//!   page template (page size, margins, card size, grid, spacing, font).
//! - A **project** is a [`FlashcardProject`] — a layout bundled with its card
//!   content, so a whole working set can be reopened and regenerated.
//!
//! [`load_flashcard_file`] reads either shape and reports which it found.

use crate::options::FlashcardOptions;
use crate::types::{Flashcard, FlashcardError, Result};
use std::path::Path;

/// A complete flashcard project: a page layout together with its card content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlashcardProject {
    /// Page layout / arrangement options.
    pub options: FlashcardOptions,
    /// The flashcards this project was saved with.
    pub cards: Vec<Flashcard>,
}

async fn write_json<T: serde::Serialize>(value: &T, path: impl AsRef<Path>) -> Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| FlashcardError::Config(format!("Failed to serialize: {e}")))?;
    tokio::fs::write(path, json).await?;
    Ok(())
}

impl FlashcardOptions {
    /// Save these layout options to a JSON file (a content-independent template).
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        write_json(self, path).await
    }
}

impl FlashcardProject {
    /// Save this project (layout + cards) to a JSON file.
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        write_json(self, path).await
    }
}

/// The two shapes [`load_flashcard_file`] can produce.
#[derive(Debug, Clone)]
pub enum LoadedLayout {
    /// A layout-only template; the caller should keep any existing cards.
    Layout(FlashcardOptions),
    /// A full project; the caller should replace its cards with these.
    Project(FlashcardProject),
}

/// Load a flashcard file, auto-detecting whether it is a full project (layout +
/// cards) or a layout-only template.
///
/// A project is tried first: its JSON object has `options` and `cards` keys that
/// a flat layout file lacks, so the two shapes are unambiguous.
pub async fn load_flashcard_file(path: impl AsRef<Path>) -> Result<LoadedLayout> {
    let bytes = tokio::fs::read(path).await?;

    if let Ok(project) = serde_json::from_slice::<FlashcardProject>(&bytes) {
        return Ok(LoadedLayout::Project(project));
    }

    let options = serde_json::from_slice::<FlashcardOptions>(&bytes)
        .map_err(|e| FlashcardError::Config(format!("Failed to parse layout file: {e}")))?;
    Ok(LoadedLayout::Layout(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn project_round_trips_with_cards() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("project.json");

        let project = FlashcardProject {
            options: FlashcardOptions::default(),
            cards: vec![
                Flashcard {
                    front: "front".into(),
                    back: "back".into(),
                },
                Flashcard {
                    front: "q".into(),
                    back: "a".into(),
                },
            ],
        };
        project.save(&path).await.unwrap();

        match load_flashcard_file(&path).await.unwrap() {
            LoadedLayout::Project(loaded) => {
                assert_eq!(loaded.cards.len(), 2);
                assert_eq!(loaded.cards[0].front, "front");
            }
            LoadedLayout::Layout(_) => panic!("expected a project, got a layout"),
        }
    }

    #[tokio::test]
    async fn layout_round_trips_without_cards() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("layout.json");

        let options = FlashcardOptions {
            rows: 4,
            columns: 5,
            ..FlashcardOptions::default()
        };
        options.save(&path).await.unwrap();

        match load_flashcard_file(&path).await.unwrap() {
            LoadedLayout::Layout(loaded) => {
                assert_eq!(loaded.rows, 4);
                assert_eq!(loaded.columns, 5);
            }
            LoadedLayout::Project(_) => panic!("expected a layout, got a project"),
        }
    }
}
