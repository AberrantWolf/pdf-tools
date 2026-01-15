//! Signature slot calculation
//!
//! This module calculates how pages are ordered within signatures for
//! traditional bookbinding layouts (folio, quarto, octavo).
//!
//! ## Traditional Bookbinding Layouts
//!
//! **Folio (4 pages, 1 fold):**
//! - Side A: [4, 1] (left=4, right=1)
//! - Side B: [2, 3] (left=2, right=3)
//! - No rotation needed
//!
//! **Quarto (8 pages, 2 folds):**
//! - Side A: Top [5↓, 4↓], Bottom [8, 1]
//! - Side B: Top [6↓, 3↓], Bottom [7, 2]
//! - Top row rotated 180°
//!
//! **Octavo (16 pages, 3 folds):**
//! - Side A: Top [5↓, 12↓, 9↓, 8↓], Bottom [4, 13, 16, 1]
//! - Side B: Top [6↓, 11↓, 10↓, 7↓], Bottom [3, 14, 15, 2]
//! - Top row rotated 180°

use crate::types::PageArrangement;

use super::{PageSide, SheetSide, SignatureSlot};

// =============================================================================
// Signature Calculation
// =============================================================================

/// Calculate signature slots for all signatures needed to hold the given pages.
///
/// Returns a vector of signatures, where each signature contains all its slots
/// in the order they appear (front side first, then back side).
pub fn calculate_signature_slots(
    total_pages: usize,
    arrangement: PageArrangement,
) -> Vec<Vec<SignatureSlot>> {
    let pages_per_sig = arrangement.pages_per_signature();

    // Pad to multiple of pages_per_signature
    let padded_count = ((total_pages + pages_per_sig - 1) / pages_per_sig) * pages_per_sig;
    let num_signatures = padded_count / pages_per_sig;

    (0..num_signatures)
        .map(|_| create_signature_slots(arrangement))
        .collect()
}

/// Create the slot layout for a single signature.
///
/// The slots are returned in sheet order: all front-side slots first,
/// then all back-side slots. Within each side, slots are in row-major order
/// (top-left to bottom-right).
fn create_signature_slots(arrangement: PageArrangement) -> Vec<SignatureSlot> {
    match arrangement {
        PageArrangement::Folio => create_folio_slots(),
        PageArrangement::Quarto => create_quarto_slots(),
        PageArrangement::Octavo => create_octavo_slots(),
        PageArrangement::Custom {
            pages_per_signature,
        } => create_custom_slots(pages_per_signature),
    }
}

// =============================================================================
// Page Ordering
// =============================================================================

/// Calculate the page order for a signature (which page number goes in each slot).
///
/// Returns 0-based page indices relative to the signature start.
/// For example, folio returns [3, 0, 1, 2] meaning:
/// - Slot 0 gets page 4 (index 3)
/// - Slot 1 gets page 1 (index 0)
/// - Slot 2 gets page 2 (index 1)
/// - Slot 3 gets page 3 (index 2)
fn calculate_page_order(arrangement: PageArrangement) -> Vec<usize> {
    match arrangement {
        PageArrangement::Folio => vec![3, 0, 1, 2],
        PageArrangement::Quarto => vec![
            4, 3, // Side A top: pages 5, 4
            7, 0, // Side A bottom: pages 8, 1
            2, 5, // Side B top: pages 3, 6 (mirrored)
            1, 6, // Side B bottom: pages 2, 7 (mirrored)
        ],
        PageArrangement::Octavo => vec![
            // Side A - top row
            4, 11, 8, 7, // Side A - bottom row
            3, 12, 15, 0, // Side B - top row (mirrored)
            5, 10, 9, 6, // Side B - bottom row (mirrored)
            2, 13, 14, 1,
        ],
        PageArrangement::Custom {
            pages_per_signature,
        } => {
            // Generic saddle-stitch pattern
            let sheets = pages_per_signature / 4;
            let mut order = Vec::with_capacity(pages_per_signature);
            for i in 0..sheets {
                let last = pages_per_signature - 1 - (2 * i);
                let first = 2 * i;
                order.push(last);
                order.push(first);
                order.push(first + 1);
                order.push(last - 1);
            }
            order
        }
    }
}

/// Map source pages to signature slots.
///
/// Given the slots for a signature and the starting page index,
/// returns which source page goes in each slot (or None for blank padding).
pub fn map_pages_to_slots(
    arrangement: PageArrangement,
    sig_start: usize,
    total_source_pages: usize,
) -> Vec<Option<usize>> {
    calculate_page_order(arrangement)
        .into_iter()
        .map(|relative_idx| {
            let absolute_idx = sig_start + relative_idx;
            if absolute_idx < total_source_pages {
                Some(absolute_idx)
            } else {
                None // Blank padding
            }
        })
        .collect()
}

/// Get slots for a specific sheet side
pub fn slots_for_side(slots: &[SignatureSlot], side: SheetSide) -> Vec<&SignatureSlot> {
    slots.iter().filter(|s| s.sheet_side == side).collect()
}

// =============================================================================
// Slot Creation - Folio
// =============================================================================

/// Create slots for folio arrangement (4 pages, 2x1 grid, 1 fold)
///
/// Layout after folding:
/// ```text
/// +---+---+
/// | 2 | 3 |  <- inside (verso, recto)
/// +---+---+
/// | 4 | 1 |  <- outside (verso, recto)  [this is Side A]
/// +---+---+
/// ```
///
/// Printed sheets:
/// - Side A (front): [page 4, page 1] left to right
/// - Side B (back):  [page 2, page 3] left to right
fn create_folio_slots() -> Vec<SignatureSlot> {
    vec![
        // Side A (front) - 2 cols x 1 row
        SignatureSlot::new(0, SheetSide::Front, 0, 0, false, PageSide::Verso), // page 4
        SignatureSlot::new(1, SheetSide::Front, 0, 1, false, PageSide::Recto), // page 1
        // Side B (back) - 2 cols x 1 row
        SignatureSlot::new(2, SheetSide::Back, 0, 0, false, PageSide::Verso), // page 2
        SignatureSlot::new(3, SheetSide::Back, 0, 1, false, PageSide::Recto), // page 3
    ]
}

// =============================================================================
// Slot Creation - Quarto
// =============================================================================

/// Create slots for quarto arrangement (8 pages, 2x2 grid, 2 folds)
///
/// Printed sheets (before mirroring for duplex):
/// - Side A: Top row [5↓, 4↓], Bottom row [8, 1]
/// - Side B: Top row [6↓, 3↓], Bottom row [7, 2]
///
/// For duplex printing, Side B is horizontally mirrored:
/// - Side B printed: Top row [3↓, 6↓], Bottom row [2, 7]
fn create_quarto_slots() -> Vec<SignatureSlot> {
    vec![
        // Side A (front) - 2 cols x 2 rows
        // Top row (rotated 180°)
        SignatureSlot::new(0, SheetSide::Front, 0, 0, true, PageSide::Recto), // page 5
        SignatureSlot::new(1, SheetSide::Front, 0, 1, true, PageSide::Verso), // page 4
        // Bottom row (not rotated)
        SignatureSlot::new(2, SheetSide::Front, 1, 0, false, PageSide::Verso), // page 8
        SignatureSlot::new(3, SheetSide::Front, 1, 1, false, PageSide::Recto), // page 1
        // Side B (back) - mirrored horizontally for duplex
        // Top row (rotated 180°)
        SignatureSlot::new(4, SheetSide::Back, 0, 0, true, PageSide::Recto), // page 3
        SignatureSlot::new(5, SheetSide::Back, 0, 1, true, PageSide::Verso), // page 6
        // Bottom row (not rotated)
        SignatureSlot::new(6, SheetSide::Back, 1, 0, false, PageSide::Recto), // page 2
        SignatureSlot::new(7, SheetSide::Back, 1, 1, false, PageSide::Verso), // page 7
    ]
}

// =============================================================================
// Slot Creation - Octavo
// =============================================================================

/// Create slots for octavo arrangement (16 pages, 4x2 grid, 3 folds)
///
/// Printed sheets:
/// - Side A: Top row [5↓, 12↓, 9↓, 8↓], Bottom row [4, 13, 16, 1]
/// - Side B (mirrored): Top row [6↓, 11↓, 10↓, 7↓], Bottom row [3, 14, 15, 2]
fn create_octavo_slots() -> Vec<SignatureSlot> {
    vec![
        // Side A (front) - 4 cols x 2 rows
        // Top row (rotated 180°)
        SignatureSlot::new(0, SheetSide::Front, 0, 0, true, PageSide::Recto), // page 5
        SignatureSlot::new(1, SheetSide::Front, 0, 1, true, PageSide::Verso), // page 12
        SignatureSlot::new(2, SheetSide::Front, 0, 2, true, PageSide::Recto), // page 9
        SignatureSlot::new(3, SheetSide::Front, 0, 3, true, PageSide::Verso), // page 8
        // Bottom row (not rotated)
        SignatureSlot::new(4, SheetSide::Front, 1, 0, false, PageSide::Verso), // page 4
        SignatureSlot::new(5, SheetSide::Front, 1, 1, false, PageSide::Recto), // page 13
        SignatureSlot::new(6, SheetSide::Front, 1, 2, false, PageSide::Verso), // page 16
        SignatureSlot::new(7, SheetSide::Front, 1, 3, false, PageSide::Recto), // page 1
        // Side B (back) - mirrored for duplex
        // Top row (rotated 180°)
        SignatureSlot::new(8, SheetSide::Back, 0, 0, true, PageSide::Verso), // page 6
        SignatureSlot::new(9, SheetSide::Back, 0, 1, true, PageSide::Recto), // page 11
        SignatureSlot::new(10, SheetSide::Back, 0, 2, true, PageSide::Verso), // page 10
        SignatureSlot::new(11, SheetSide::Back, 0, 3, true, PageSide::Recto), // page 7
        // Bottom row (not rotated)
        SignatureSlot::new(12, SheetSide::Back, 1, 0, false, PageSide::Recto), // page 3
        SignatureSlot::new(13, SheetSide::Back, 1, 1, false, PageSide::Verso), // page 14
        SignatureSlot::new(14, SheetSide::Back, 1, 2, false, PageSide::Recto), // page 15
        SignatureSlot::new(15, SheetSide::Back, 1, 3, false, PageSide::Verso), // page 2
    ]
}

// =============================================================================
// Slot Creation - Custom
// =============================================================================

/// Create slots for custom page count using generic saddle-stitch pattern
fn create_custom_slots(pages_per_signature: usize) -> Vec<SignatureSlot> {
    let sheets = pages_per_signature / 4;
    let mut slots = Vec::with_capacity(pages_per_signature);

    for i in 0..sheets {
        let base_idx = i * 4;

        // Front side
        slots.push(SignatureSlot::new(
            base_idx,
            SheetSide::Front,
            0,
            0,
            false,
            PageSide::Verso,
        ));
        slots.push(SignatureSlot::new(
            base_idx + 1,
            SheetSide::Front,
            0,
            1,
            false,
            PageSide::Recto,
        ));

        // Back side
        slots.push(SignatureSlot::new(
            base_idx + 2,
            SheetSide::Back,
            0,
            0,
            false,
            PageSide::Verso,
        ));
        slots.push(SignatureSlot::new(
            base_idx + 3,
            SheetSide::Back,
            0,
            1,
            false,
            PageSide::Recto,
        ));
    }

    slots
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folio_page_order() {
        let order = calculate_page_order(PageArrangement::Folio);
        assert_eq!(order, vec![3, 0, 1, 2]);
    }

    #[test]
    fn test_folio_slots() {
        let slots = create_folio_slots();
        assert_eq!(slots.len(), 4);

        // Check front side
        assert_eq!(slots[0].sheet_side, SheetSide::Front);
        assert_eq!(slots[0].grid_pos.col, 0);
        assert!(!slots[0].rotated);

        assert_eq!(slots[1].sheet_side, SheetSide::Front);
        assert_eq!(slots[1].grid_pos.col, 1);
        assert!(!slots[1].rotated);

        // Check back side
        assert_eq!(slots[2].sheet_side, SheetSide::Back);
        assert_eq!(slots[3].sheet_side, SheetSide::Back);
    }

    #[test]
    fn test_quarto_rotation() {
        let slots = create_quarto_slots();

        // Top row should be rotated
        assert!(slots[0].rotated); // top-left front
        assert!(slots[1].rotated); // top-right front
        assert!(slots[4].rotated); // top-left back
        assert!(slots[5].rotated); // top-right back

        // Bottom row should not be rotated
        assert!(!slots[2].rotated); // bottom-left front
        assert!(!slots[3].rotated); // bottom-right front
        assert!(!slots[6].rotated); // bottom-left back
        assert!(!slots[7].rotated); // bottom-right back
    }

    #[test]
    fn test_page_mapping_with_padding() {
        // 6 source pages, folio needs 8 (2 signatures)
        let mapped = map_pages_to_slots(PageArrangement::Folio, 4, 6);

        // Second signature: pages 5, 6 exist, 7, 8 are blank
        assert_eq!(mapped[0], None); // page 8 (index 7) - blank
        assert_eq!(mapped[1], Some(4)); // page 5 (index 4)
        assert_eq!(mapped[2], Some(5)); // page 6 (index 5)
        assert_eq!(mapped[3], None); // page 7 (index 6) - blank
    }

    #[test]
    fn test_slots_for_side() {
        let slots = create_quarto_slots();

        let front = slots_for_side(&slots, SheetSide::Front);
        assert_eq!(front.len(), 4);
        assert!(front.iter().all(|s| s.sheet_side == SheetSide::Front));

        let back = slots_for_side(&slots, SheetSide::Back);
        assert_eq!(back.len(), 4);
        assert!(back.iter().all(|s| s.sheet_side == SheetSide::Back));
    }

    #[test]
    fn test_signature_slot_new() {
        let slot = SignatureSlot::new(5, SheetSide::Back, 1, 2, true, PageSide::Verso);

        assert_eq!(slot.slot_index, 5);
        assert_eq!(slot.sheet_side, SheetSide::Back);
        assert_eq!(slot.grid_pos.row, 1);
        assert_eq!(slot.grid_pos.col, 2);
        assert!(slot.rotated);
        assert_eq!(slot.page_side, PageSide::Verso);
        assert_eq!(slot.rotation_degrees(), 180.0);
    }
}
