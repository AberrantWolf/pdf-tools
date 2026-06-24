//! A lightweight production summary for a flashcard layout: how many sheets a
//! deck will fill, how many slots go unused, and whether the grid overflows the
//! printable area. Computed in the library so the CLI and GUI render the same
//! numbers without duplicating layout math.

use crate::options::{Duplex, FlashcardOptions};
use crate::types::FlashcardWarning;

/// Tolerance (mm) when checking whether the grid fits the printable area, to
/// avoid spurious warnings from floating-point rounding.
const FIT_TOLERANCE_MM: f32 = 0.01;

/// A summary of how a given card count lays out under some [`FlashcardOptions`].
#[derive(Debug, Clone, PartialEq)]
pub struct FlashcardSummary {
    /// Total number of cards.
    pub card_count: usize,
    /// Cards that fit on one sheet (`rows × columns`).
    pub cards_per_sheet: usize,
    /// Physical sheets of paper needed.
    pub sheet_count: usize,
    /// PDF pages produced (`sheet_count`, doubled when two-sided).
    pub pdf_page_count: usize,
    /// Unused card slots on the final sheet.
    pub leftover_slots: usize,
    /// Whether backs are printed (false for [`Duplex::OneSided`]).
    pub double_sided: bool,
    /// Layout-fit warnings (e.g. the grid overflows the printable area).
    pub warnings: Vec<FlashcardWarning>,
}

impl FlashcardOptions {
    /// Summarize how `card_count` cards lay out under these options.
    pub fn summarize(&self, card_count: usize) -> FlashcardSummary {
        let cards_per_sheet = (self.rows * self.columns).max(1);
        let sheet_count = card_count.div_ceil(cards_per_sheet);
        let double_sided = self.duplex != Duplex::OneSided;
        let pdf_page_count = sheet_count * if double_sided { 2 } else { 1 };

        let leftover_slots = if card_count == 0 {
            0
        } else {
            (cards_per_sheet - 1) - (card_count - 1) % cards_per_sheet
        };

        let mut warnings = Vec::new();
        let grid_width = self.columns as f32 * self.card_width_mm
            + self.columns.saturating_sub(1) as f32 * self.column_spacing_mm;
        let printable_width = self.page_width_mm - self.margin_left_mm - self.margin_right_mm;
        if grid_width > printable_width + FIT_TOLERANCE_MM {
            warnings.push(FlashcardWarning::GridWiderThanPrintable);
        }
        let grid_height = self.rows as f32 * self.card_height_mm
            + self.rows.saturating_sub(1) as f32 * self.row_spacing_mm;
        let printable_height = self.page_height_mm - self.margin_top_mm - self.margin_bottom_mm;
        if grid_height > printable_height + FIT_TOLERANCE_MM {
            warnings.push(FlashcardWarning::GridTallerThanPrintable);
        }

        FlashcardSummary {
            card_count,
            cards_per_sheet,
            sheet_count,
            pdf_page_count,
            leftover_slots,
            double_sided,
            warnings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layout that comfortably fits: 2×2 of small cards on Letter.
    fn fitting_options() -> FlashcardOptions {
        FlashcardOptions {
            rows: 2,
            columns: 2,
            card_width_mm: 60.0,
            card_height_mm: 90.0,
            row_spacing_mm: 5.0,
            column_spacing_mm: 5.0,
            margin_left_mm: 10.0,
            margin_right_mm: 10.0,
            margin_top_mm: 10.0,
            margin_bottom_mm: 10.0,
            ..FlashcardOptions::default()
        }
    }

    #[test]
    fn counts_sheets_and_leftovers() {
        let opts = fitting_options(); // 4 per sheet, double-sided
        let s = opts.summarize(10);
        assert_eq!(s.cards_per_sheet, 4);
        assert_eq!(s.sheet_count, 3);
        assert_eq!(s.pdf_page_count, 6);
        assert_eq!(s.leftover_slots, 2); // 10 cards -> 2 unused on the 3rd sheet
        assert!(s.double_sided);
    }

    #[test]
    fn exact_fill_has_no_leftovers() {
        let s = fitting_options().summarize(8);
        assert_eq!(s.sheet_count, 2);
        assert_eq!(s.leftover_slots, 0);
    }

    #[test]
    fn empty_deck_is_zero() {
        let s = fitting_options().summarize(0);
        assert_eq!(s.sheet_count, 0);
        assert_eq!(s.pdf_page_count, 0);
        assert_eq!(s.leftover_slots, 0);
    }

    #[test]
    fn one_sided_halves_pages() {
        let opts = FlashcardOptions {
            duplex: Duplex::OneSided,
            ..fitting_options()
        };
        let s = opts.summarize(8);
        assert_eq!(s.sheet_count, 2);
        assert_eq!(s.pdf_page_count, 2);
        assert!(!s.double_sided);
    }

    #[test]
    fn flags_overflowing_grid() {
        // Default 3×63.5mm columns on Letter overflow the printable width.
        let s = FlashcardOptions::default().summarize(6);
        assert!(
            s.warnings
                .contains(&FlashcardWarning::GridWiderThanPrintable)
        );
    }
}
