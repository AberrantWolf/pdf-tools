//! Saving and loading flashcard files.
//!
//! Two on-disk shapes are supported, both JSON:
//!
//! - A **layout** is just a [`FlashcardOptions`] — a reusable, content-independent
//!   page template (page size, margins, card size, grid, spacing, font, duplex).
//! - A **deck** is a [`FlashcardDeck`] — a layout bundled with its card content,
//!   so a whole working set can be reopened and regenerated.
//!
//! ("Deck" is deliberately distinct from the app-level *project* `.pjproj`, which
//! persists every mode's settings and file paths rather than card content.)
//!
//! [`load_flashcard_file`] reads either shape and reports which it found.

use crate::options::FlashcardOptions;
use crate::types::{Flashcard, FlashcardError, Result};
use std::path::Path;

/// A complete flashcard deck: a page layout together with its card content.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FlashcardDeck {
    /// Page layout / arrangement options.
    pub options: FlashcardOptions,
    /// The flashcards this deck was saved with.
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

impl FlashcardDeck {
    /// Save this deck (layout + cards) to a JSON file.
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        write_json(self, path).await
    }
}

/// The two shapes [`load_flashcard_file`] can produce.
#[derive(Debug, Clone)]
pub enum LoadedFile {
    /// A layout-only template; the caller should keep any existing cards.
    Layout(FlashcardOptions),
    /// A full deck; the caller should replace its cards with these.
    Deck(FlashcardDeck),
}

/// Load a flashcard file, auto-detecting whether it is a full deck (layout +
/// cards) or a layout-only template.
///
/// A deck is tried first: its JSON object has `options` and `cards` keys that a
/// flat layout file lacks, so the two shapes are unambiguous.
pub async fn load_flashcard_file(path: impl AsRef<Path>) -> Result<LoadedFile> {
    let bytes = tokio::fs::read(path).await?;

    if let Ok(deck) = serde_json::from_slice::<FlashcardDeck>(&bytes) {
        return Ok(LoadedFile::Deck(deck));
    }

    let options = serde_json::from_slice::<FlashcardOptions>(&bytes)
        .map_err(|e| FlashcardError::Config(format!("Failed to parse layout file: {e}")))?;
    Ok(LoadedFile::Layout(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deck_round_trips_with_cards() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deck.json");

        let deck = FlashcardDeck {
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
        deck.save(&path).await.unwrap();

        match load_flashcard_file(&path).await.unwrap() {
            LoadedFile::Deck(loaded) => {
                assert_eq!(loaded.cards.len(), 2);
                assert_eq!(loaded.cards[0].front, "front");
            }
            LoadedFile::Layout(_) => panic!("expected a deck, got a layout"),
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
            LoadedFile::Layout(loaded) => {
                assert_eq!(loaded.rows, 4);
                assert_eq!(loaded.columns, 5);
            }
            LoadedFile::Deck(_) => panic!("expected a layout, got a deck"),
        }
    }

    #[tokio::test]
    async fn layout_saved_before_duplex_and_split_fonts_still_loads() {
        // A legacy layout JSON predates both the `duplex` field and per-side font
        // sizes: it must deserialize (serde defaults), its single `font_size_pt`
        // mapping onto the front size and the back falling back to the default.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old_layout.json");
        let legacy = r#"{
            "page_width_mm": 215.9, "page_height_mm": 279.4,
            "margin_top_mm": 10, "margin_bottom_mm": 10,
            "margin_left_mm": 10, "margin_right_mm": 10,
            "card_width_mm": 60, "card_height_mm": 90,
            "rows": 2, "columns": 2,
            "row_spacing_mm": 5, "column_spacing_mm": 5,
            "font_size_pt": 18
        }"#;
        tokio::fs::write(&path, legacy).await.unwrap();

        match load_flashcard_file(&path).await.unwrap() {
            LoadedFile::Layout(o) => {
                assert_eq!(o.duplex, crate::options::Duplex::LongEdge);
                assert!((o.front_font_size_pt - 18.0).abs() < f32::EPSILON);
                let default_back = FlashcardOptions::default().back_font_size_pt;
                assert!((o.back_font_size_pt - default_back).abs() < f32::EPSILON);
            }
            LoadedFile::Deck(_) => panic!("expected a layout"),
        }
    }
}
