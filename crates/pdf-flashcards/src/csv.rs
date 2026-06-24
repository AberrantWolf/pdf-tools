use crate::types::{Flashcard, FlashcardWarning, Result};
use std::path::Path;

/// Load flashcards from a CSV file on disk.
///
/// Each row must have at least two columns (`front`, `back`); the first row is
/// treated as a header and skipped. Rows with fewer than two columns are
/// skipped with a warning rather than failing the whole load.
pub async fn load_from_csv(
    path: impl AsRef<Path>,
) -> Result<(Vec<Flashcard>, Vec<FlashcardWarning>)> {
    let path = path.as_ref().to_owned();

    log::info!("Loading flashcards from {}", path.display());
    let contents = tokio::fs::read_to_string(&path).await?;

    let (cards, warnings) =
        tokio::task::spawn_blocking(move || parse_records(&contents, b',', true)).await?;

    log::info!("Loaded {} flashcards", cards.len());

    Ok((cards, warnings))
}

/// Parse flashcards from pasted text (one card per line, `front<delimiter>back`).
///
/// The delimiter is auto-detected: tab if any line contains one, otherwise
/// comma. Unlike CSV files, every row is treated as data — no header is
/// skipped — so the first pasted line is not silently dropped.
pub fn parse_cards(text: &str) -> (Vec<Flashcard>, Vec<FlashcardWarning>) {
    let delimiter = if text.lines().any(|line| line.contains('\t')) {
        b'\t'
    } else {
        b','
    };
    parse_records(text, delimiter, false)
}

/// Shared CSV/TSV parsing used by both file loading and pasted-text input.
fn parse_records(
    text: &str,
    delimiter: u8,
    has_headers: bool,
) -> (Vec<Flashcard>, Vec<FlashcardWarning>) {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_headers)
        .flexible(true)
        .from_reader(text.as_bytes());

    let mut cards = Vec::new();
    let mut warnings = Vec::new();
    // Row numbers are 1-indexed for humans; offset by an extra row when a
    // header was consumed so reported numbers line up with the source file.
    let row_offset = if has_headers { 2 } else { 1 };

    for (i, result) in reader.records().enumerate() {
        let record = match result {
            Ok(record) => record,
            Err(e) => {
                log::warn!("Skipping unreadable row: {e}");
                continue;
            }
        };
        let row_number = i + row_offset;
        if record.len() >= 2 {
            cards.push(Flashcard {
                front: record[0].to_string(),
                back: record[1].to_string(),
            });
        } else {
            log::warn!(
                "Row {}: skipping (has {} column(s), need >= 2)",
                row_number,
                record.len()
            );
            warnings.push(FlashcardWarning::CsvRowSkipped {
                row_number,
                column_count: record.len(),
            });
        }
    }

    if cards.is_empty() {
        warnings.push(FlashcardWarning::EmptyCsv);
    }

    (cards, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_comma_separated_paste() {
        let (cards, _) = parse_cards("hello,world\nfoo,bar");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].front, "hello");
        assert_eq!(cards[0].back, "world");
        assert_eq!(cards[1].back, "bar");
    }

    #[test]
    fn parses_tab_separated_paste() {
        let (cards, _) = parse_cards("hello\tworld\nfoo\tbar");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].back, "world");
    }

    #[test]
    fn paste_keeps_first_row() {
        // No header is skipped for pasted text.
        let (cards, _) = parse_cards("first,card\nsecond,card");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].front, "first");
    }

    #[test]
    fn paste_handles_quoted_commas() {
        let (cards, _) = parse_cards("\"a, b\",\"c, d\"");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].front, "a, b");
        assert_eq!(cards[0].back, "c, d");
    }

    #[test]
    fn empty_input_warns() {
        let (cards, warnings) = parse_cards("");
        assert!(cards.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, FlashcardWarning::EmptyCsv))
        );
    }

    #[test]
    fn single_column_row_is_skipped() {
        let (cards, warnings) = parse_cards("lonely");
        assert!(cards.is_empty());
        assert!(warnings.iter().any(|w| matches!(
            w,
            FlashcardWarning::CsvRowSkipped {
                column_count: 1,
                ..
            }
        )));
    }
}
