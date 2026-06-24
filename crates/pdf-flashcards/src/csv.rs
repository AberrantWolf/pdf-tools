use crate::table::CsvTable;
use crate::types::{FlashcardWarning, Result};
use std::path::Path;

/// Load a CSV file into a [`CsvTable`].
///
/// With `has_headers`, the first row names the columns; otherwise every row is
/// data and columns are named `Col1`..`ColN`. Mapping columns to card sides is a
/// separate step (see [`CsvTable::to_cards`]).
pub async fn load_table_from_csv(
    path: impl AsRef<Path>,
    has_headers: bool,
) -> Result<(CsvTable, Vec<FlashcardWarning>)> {
    let path = path.as_ref().to_owned();

    log::info!("Loading flashcard table from {}", path.display());
    let contents = tokio::fs::read_to_string(&path).await?;

    let (table, warnings) =
        tokio::task::spawn_blocking(move || parse_table(&contents, b',', has_headers)).await?;

    log::info!(
        "Loaded {} column(s), {} row(s)",
        table.columns.len(),
        table.rows.len()
    );

    Ok((table, warnings))
}

/// Parse pasted text into a [`CsvTable`], auto-detecting the delimiter (tab if
/// any line contains one, else comma).
pub fn parse_pasted_table(text: &str, has_headers: bool) -> (CsvTable, Vec<FlashcardWarning>) {
    let delimiter = if text.lines().any(|line| line.contains('\t')) {
        b'\t'
    } else {
        b','
    };
    parse_table(text, delimiter, has_headers)
}

/// Parse delimited text into a [`CsvTable`]. Shared by file loading and pasted
/// input. Rows are kept as-is (ragged allowed); only a wholly empty source
/// warns.
pub fn parse_table(
    text: &str,
    delimiter: u8,
    has_headers: bool,
) -> (CsvTable, Vec<FlashcardWarning>) {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(has_headers)
        .flexible(true)
        .from_reader(text.as_bytes());

    // Header values name the columns when present (owned now, before the
    // mutable `records()` borrow below).
    let mut columns: Vec<String> = if has_headers {
        reader
            .headers()
            .map(|h| h.iter().map(str::to_string).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut rows: Vec<Vec<String>> = Vec::new();
    for result in reader.records() {
        match result {
            Ok(record) => rows.push(record.iter().map(str::to_string).collect()),
            Err(e) => log::warn!("Skipping unreadable row: {e}"),
        }
    }

    // Expose a column for every cell the source has: the header width and the
    // widest data row. Unnamed columns become `Col1`..`ColN` (1-based).
    let width = columns
        .len()
        .max(rows.iter().map(Vec::len).max().unwrap_or(0));
    for i in columns.len()..width {
        columns.push(format!("Col{}", i + 1));
    }

    let mut warnings = Vec::new();
    if rows.is_empty() {
        warnings.push(FlashcardWarning::EmptyCsv);
    }

    (CsvTable { columns, rows }, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::ColumnRole;

    #[test]
    fn header_row_names_columns() {
        let (table, _) = parse_table("word,meaning\nchien,dog\nchat,cat", b',', true);
        assert_eq!(table.columns, vec!["word", "meaning"]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0], vec!["chien", "dog"]);
    }

    #[test]
    fn no_header_falls_back_to_col_names_1_based() {
        let (table, _) = parse_table("chien,dog\nchat,cat", b',', false);
        assert_eq!(table.columns, vec!["Col1", "Col2"]);
        assert_eq!(table.rows.len(), 2); // first row is data, not a header
        assert_eq!(table.rows[0], vec!["chien", "dog"]);
    }

    #[test]
    fn ragged_rows_expose_every_column() {
        let (table, _) = parse_table("a,b\nx,y,z", b',', true);
        // The widest row (3 cells) adds a third, auto-named column.
        assert_eq!(table.columns, vec!["a", "b", "Col3"]);
        assert_eq!(table.rows[0], vec!["x", "y", "z"]);
    }

    #[test]
    fn pasted_text_autodetects_tab_and_keeps_first_row() {
        let (table, _) = parse_pasted_table("hello\tworld\nfoo\tbar", false);
        assert_eq!(table.columns, vec!["Col1", "Col2"]);
        assert_eq!(table.rows.len(), 2);
        let cards = table.to_cards(&table.default_roles(), "\n");
        assert_eq!(cards[0].front, "hello");
        assert_eq!(cards[0].back, "world");
    }

    #[test]
    fn quoted_commas_are_one_cell() {
        let (table, _) = parse_table("\"a, b\",\"c, d\"", b',', false);
        assert_eq!(table.rows[0], vec!["a, b", "c, d"]);
    }

    #[test]
    fn empty_input_warns() {
        let (table, warnings) = parse_table("", b',', true);
        assert!(table.rows.is_empty());
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w, FlashcardWarning::EmptyCsv))
        );
    }

    #[test]
    fn default_role_projection_matches_legacy_first_two_columns() {
        let (table, _) = parse_table("front,back,note\nq,a,ignored", b',', true);
        let cards = table.to_cards(&table.default_roles(), "\n");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].front, "q");
        assert_eq!(cards[0].back, "a");
        assert_eq!(table.default_roles()[2], ColumnRole::Ignore);
    }
}
