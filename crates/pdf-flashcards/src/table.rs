//! The parsed CSV/TSV table and the column→card mapping.
//!
//! Importing is two steps: parse the source into a [`CsvTable`] (named columns +
//! rows), then project it into [`Flashcard`]s with a per-column [`ColumnRole`].
//! Keeping the table around lets the UI re-map columns without re-reading the
//! file.

use crate::types::Flashcard;

/// A parsed CSV/TSV table: named columns and their data rows.
#[derive(Debug, Clone, Default)]
pub struct CsvTable {
    /// Column names — the header row's values, or `Col1`..`ColN` (1-based) when
    /// the source has no header.
    pub columns: Vec<String>,
    /// Data rows; each is a list of cell values. Rows may be shorter than
    /// `columns` (ragged sources); missing cells project as empty.
    pub rows: Vec<Vec<String>>,
}

/// Which side of the card a column feeds, or whether it's left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ColumnRole {
    Front,
    Back,
    #[default]
    Ignore,
}

impl CsvTable {
    /// The default mapping: the first column feeds the front, the second the
    /// back, the rest are ignored — matching the historical "first two columns"
    /// behaviour. Used for a freshly loaded table and by the CLI.
    pub fn default_roles(&self) -> Vec<ColumnRole> {
        (0..self.columns.len())
            .map(|i| match i {
                0 => ColumnRole::Front,
                1 => ColumnRole::Back,
                _ => ColumnRole::Ignore,
            })
            .collect()
    }

    /// Build flashcards by joining, with `separator`, the columns assigned to
    /// each side. `roles` is indexed by column; columns beyond its length (or
    /// cells beyond a short row) contribute nothing.
    pub fn to_cards(&self, roles: &[ColumnRole], separator: &str) -> Vec<Flashcard> {
        self.rows
            .iter()
            .map(|row| Flashcard {
                front: join_side(row, roles, ColumnRole::Front, separator),
                back: join_side(row, roles, ColumnRole::Back, separator),
            })
            .collect()
    }
}

/// Join the cells of `row` whose column has role `want`, in column order.
fn join_side(row: &[String], roles: &[ColumnRole], want: ColumnRole, separator: &str) -> String {
    roles
        .iter()
        .enumerate()
        .filter(|&(_, &role)| role == want)
        .filter_map(|(col, _)| row.get(col))
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> CsvTable {
        CsvTable {
            columns: vec!["word".into(), "ipa".into(), "meaning".into()],
            rows: vec![
                vec!["chien".into(), "ʃjɛ̃".into(), "dog".into()],
                vec!["chat".into(), "ʃa".into(), "cat".into()],
            ],
        }
    }

    #[test]
    fn default_roles_use_first_two_columns() {
        let t = table();
        assert_eq!(
            t.default_roles(),
            vec![ColumnRole::Front, ColumnRole::Back, ColumnRole::Ignore]
        );
        let cards = t.to_cards(&t.default_roles(), "\n");
        assert_eq!(cards[0].front, "chien"); // col 0
        assert_eq!(cards[0].back, "ʃjɛ̃"); // col 1; col 2 (meaning) is ignored
    }

    #[test]
    fn multiple_columns_per_side_join_with_separator() {
        let t = table();
        let roles = vec![ColumnRole::Front, ColumnRole::Front, ColumnRole::Back];
        let cards = t.to_cards(&roles, "\n");
        assert_eq!(cards[0].front, "chien\nʃjɛ̃");
        assert_eq!(cards[0].back, "dog");
    }

    #[test]
    fn short_rows_and_extra_roles_are_tolerated() {
        let t = CsvTable {
            columns: vec!["a".into(), "b".into()],
            rows: vec![vec!["only-front".into()]],
        };
        let roles = vec![ColumnRole::Front, ColumnRole::Back];
        let cards = t.to_cards(&roles, "\n");
        assert_eq!(cards[0].front, "only-front");
        assert_eq!(cards[0].back, ""); // missing cell → empty side
    }
}
