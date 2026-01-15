//! Content placement within cells
//!
//! This module handles calculating the final position of page content
//! within grid cells, accounting for:
//! - Leaf margins (spine, fore-edge, top, bottom, cut)
//! - Content alignment toward folds
//! - Scaling

use crate::constants::{DEFAULT_PAGE_DIMENSIONS, mm_to_pt};
use crate::types::{LeafMargins, ScalingMode};

use super::{GridLayout, PagePlacement, Rect, SignatureSlot, cell_bounds, cell_edge_info};

// =============================================================================
// Content Area Calculation
// =============================================================================

/// Calculate the content area within a cell, accounting for margins.
///
/// The content area is the region where page content can be placed,
/// after applying leaf margins. Margins are applied based on edge type:
/// - Spine margin: the binding edge (center fold for signatures)
/// - Fore-edge margin: the outer edge opposite the spine
/// - Cut margin: edges where pages will be cut apart
/// - Top/bottom margins: head and tail of the page
///
/// # Arguments
/// * `cell` - The cell bounds
/// * `margins` - Leaf margins configuration
/// * `slot` - The signature slot (contains rotation and page side info)
/// * `grid` - The grid layout (for determining fold/cut positions)
pub fn calculate_content_area(
    cell: &Rect,
    margins: &LeafMargins,
    slot: &SignatureSlot,
    grid: &GridLayout,
) -> Rect {
    let edges = cell_edge_info(grid, slot.grid_pos);

    // Calculate margin for each edge based on what's there.
    // Priority: cut > outer (fore-edge) > spine fold > non-spine fold (0)
    let margin_left = calculate_edge_margin(
        margins,
        edges.cut_left,
        edges.outer_left,
        edges.fold_left,
        edges.is_spine_left(),
    );

    let margin_right = calculate_edge_margin(
        margins,
        edges.cut_right,
        edges.outer_right,
        edges.fold_right,
        edges.is_spine_right(),
    );

    let margin_top = calculate_edge_margin(
        margins,
        edges.cut_top,
        edges.outer_top,
        edges.fold_top,
        edges.is_spine_top(),
    );

    let margin_bottom = calculate_edge_margin(
        margins,
        edges.cut_bottom,
        edges.outer_bottom,
        edges.fold_bottom,
        edges.is_spine_bottom(),
    );

    // Convert margins from mm to points and inset the cell
    cell.inset(
        mm_to_pt(margin_left),
        mm_to_pt(margin_bottom),
        mm_to_pt(margin_right),
        mm_to_pt(margin_top),
    )
}

/// Calculate the margin for a single edge based on its properties.
fn calculate_edge_margin(
    margins: &LeafMargins,
    is_cut: bool,
    is_outer: bool,
    is_fold: bool,
    is_spine: bool,
) -> f32 {
    if is_cut {
        margins.cut_mm
    } else if is_outer {
        margins.fore_edge_mm
    } else if is_fold && is_spine {
        margins.spine_mm
    } else {
        // Non-spine fold or interior edge: content aligns to it
        0.0
    }
}

// =============================================================================
// Page Placement
// =============================================================================

/// Calculate the final page placement within the content area.
///
/// This handles:
/// - Scaling the source page to fit the content area
/// - Aligning content toward fold edges
///
/// # Arguments
/// * `content_area` - The available area for content (after margins)
/// * `source_width` - Width of the source page in points
/// * `source_height` - Height of the source page in points
/// * `scaling_mode` - How to scale the source page
/// * `slot` - The signature slot
/// * `grid` - The grid layout
pub fn place_page(
    content_area: &Rect,
    source_width: f32,
    source_height: f32,
    scaling_mode: ScalingMode,
    slot: &SignatureSlot,
    grid: &GridLayout,
) -> PagePlacement {
    let scale = calculate_scale(
        source_width,
        source_height,
        content_area.width,
        content_area.height,
        scaling_mode,
    );

    let scaled_width = source_width * scale;
    let scaled_height = source_height * scale;

    // Determine alignment based on fold positions
    let (x, y) = calculate_alignment(content_area, scaled_width, scaled_height, slot, grid);

    PagePlacement {
        source_page: None, // Will be filled in by caller
        content_rect: Rect::new(x, y, scaled_width, scaled_height),
        rotation_degrees: slot.rotation_degrees(),
        scale,
        slot: slot.clone(),
    }
}

/// Calculate content alignment based on fold positions.
///
/// Content is pushed toward folds (where pages meet after folding)
/// for proper alignment in the bound book.
fn calculate_alignment(
    content_area: &Rect,
    scaled_width: f32,
    scaled_height: f32,
    slot: &SignatureSlot,
    grid: &GridLayout,
) -> (f32, f32) {
    let fold_right = grid.has_fold_right(slot.grid_pos.col);
    let fold_left = grid.has_fold_left(slot.grid_pos.col);
    let fold_bottom = grid.has_fold_bottom(slot.grid_pos.row);
    let fold_top = grid.has_fold_top(slot.grid_pos.row);

    // Horizontal alignment
    let x = if fold_right && !fold_left {
        // Fold on right only: push content right
        content_area.right() - scaled_width
    } else if fold_left && !fold_right {
        // Fold on left only: push content left
        content_area.x
    } else {
        // No fold preference or both sides: center
        content_area.x + (content_area.width - scaled_width) / 2.0
    };

    // Vertical alignment
    let y = if fold_bottom && !fold_top {
        // Fold at bottom only: push content down
        content_area.y
    } else if fold_top && !fold_bottom {
        // Fold at top only: push content up
        content_area.top() - scaled_height
    } else {
        // No fold preference or both: center
        content_area.y + (content_area.height - scaled_height) / 2.0
    };

    (x, y)
}

/// Calculate all page placements for a signature side.
///
/// # Arguments
/// * `grid` - The grid layout
/// * `slots` - Signature slots for this sheet side
/// * `source_pages` - Source page indices for each slot (None = blank)
/// * `source_dimensions` - (width, height) in points for each source page
/// * `leaf_margins` - Margin configuration
/// * `scaling_mode` - How to scale pages
/// * `leaf_origin` - Bottom-left corner of the leaf area
pub fn calculate_placements(
    grid: &GridLayout,
    slots: &[&SignatureSlot],
    source_pages: &[Option<usize>],
    source_dimensions: &[(f32, f32)],
    leaf_margins: &LeafMargins,
    scaling_mode: ScalingMode,
    leaf_origin: (f32, f32),
) -> Vec<PagePlacement> {
    slots
        .iter()
        .zip(source_pages.iter())
        .map(|(slot, &source_page)| {
            let cell = cell_bounds(grid, slot.grid_pos, leaf_origin);
            let content_area = calculate_content_area(&cell, leaf_margins, slot, grid);

            // Get source dimensions (use default if blank)
            let (src_width, src_height) = source_page
                .and_then(|idx| source_dimensions.get(idx).copied())
                .unwrap_or(DEFAULT_PAGE_DIMENSIONS);

            let mut placement = place_page(
                &content_area,
                src_width,
                src_height,
                scaling_mode,
                slot,
                grid,
            );
            placement.source_page = source_page;
            placement
        })
        .collect()
}

// =============================================================================
// Scaling
// =============================================================================

/// Calculate scale factor for fitting source to target dimensions.
fn calculate_scale(
    src_width: f32,
    src_height: f32,
    target_width: f32,
    target_height: f32,
    mode: ScalingMode,
) -> f32 {
    let scale_w = target_width / src_width;
    let scale_h = target_height / src_height;

    match mode {
        ScalingMode::Fit => scale_w.min(scale_h),
        ScalingMode::Fill => scale_w.max(scale_h),
        ScalingMode::None => 1.0,
        ScalingMode::Stretch => scale_w, // Use width scaling, ignore height
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{GridPosition, PageSide, SheetSide};
    use crate::types::PageArrangement;

    fn make_slot(row: usize, col: usize, rotated: bool) -> SignatureSlot {
        SignatureSlot {
            slot_index: 0,
            sheet_side: SheetSide::Front,
            grid_pos: GridPosition::new(row, col),
            rotated,
            page_side: PageSide::Recto,
        }
    }

    fn make_grid(arrangement: PageArrangement) -> GridLayout {
        super::super::create_grid_layout(arrangement, 800.0, 600.0, 850.0, 650.0)
    }

    #[test]
    fn test_content_area_with_margins() {
        let cell = Rect::new(0.0, 0.0, 400.0, 600.0);
        let margins = LeafMargins {
            top_mm: 5.0,
            bottom_mm: 5.0,
            fore_edge_mm: 5.0,
            spine_mm: 10.0,
            cut_mm: 0.0,
        };

        let grid = make_grid(PageArrangement::Folio);

        // Left cell (col 0): fold on right = spine on right
        let slot = make_slot(0, 0, false);
        let area = calculate_content_area(&cell, &margins, &slot, &grid);

        // Left margin should be fore-edge (5mm), right should be spine (10mm)
        let fore_edge_pt = mm_to_pt(5.0);
        let spine_pt = mm_to_pt(10.0);

        assert!((area.x - fore_edge_pt).abs() < 0.01);
        assert!((area.width - (400.0 - fore_edge_pt - spine_pt)).abs() < 0.01);
    }

    #[test]
    fn test_rotation_does_not_affect_margins() {
        // Margins are applied to the cell, not the content
        // So rotation should not change the content area
        let cell = Rect::new(0.0, 0.0, 400.0, 600.0);
        let margins = LeafMargins {
            top_mm: 5.0,
            bottom_mm: 5.0,
            fore_edge_mm: 5.0,
            spine_mm: 10.0,
            cut_mm: 0.0,
        };

        // Use portrait dimensions (height > width) so spine is vertical
        let grid =
            super::super::create_grid_layout(PageArrangement::Quarto, 600.0, 800.0, 650.0, 850.0);

        // Top-left cell, not rotated
        let slot_normal = make_slot(0, 0, false);
        let area_normal = calculate_content_area(&cell, &margins, &slot_normal, &grid);

        // Top-left cell, rotated
        let slot_rotated = make_slot(0, 0, true);
        let area_rotated = calculate_content_area(&cell, &margins, &slot_rotated, &grid);

        // Content area should be the same regardless of rotation
        assert!(
            (area_normal.x - area_rotated.x).abs() < 0.01,
            "Content areas should match: normal.x={}, rotated.x={}",
            area_normal.x,
            area_rotated.x
        );
        assert!(
            (area_normal.width - area_rotated.width).abs() < 0.01,
            "Content areas should match: normal.width={}, rotated.width={}",
            area_normal.width,
            area_rotated.width
        );
    }

    #[test]
    fn test_scale_fit() {
        // Source is 800x600, target is 400x400
        // To fit, we need to scale by 0.5 (width-limited)
        let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::Fit);
        assert!((scale - 0.5).abs() < 0.001);

        // Source is 400x800, target is 400x400
        // To fit, we need to scale by 0.5 (height-limited)
        let scale = calculate_scale(400.0, 800.0, 400.0, 400.0, ScalingMode::Fit);
        assert!((scale - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_scale_fill() {
        // Source is 800x600, target is 400x400
        // To fill, we need to scale by 0.667 (height-limited, will crop width)
        let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::Fill);
        assert!((scale - 400.0 / 600.0).abs() < 0.001);
    }

    #[test]
    fn test_alignment_toward_fold() {
        let content_area = Rect::new(10.0, 10.0, 400.0, 600.0);
        let grid = make_grid(PageArrangement::Folio);

        // Left cell (col 0): fold on right, content should be pushed right
        let slot_left = make_slot(0, 0, false);
        let placement = place_page(
            &content_area,
            300.0, // Smaller than content area
            500.0,
            ScalingMode::None,
            &slot_left,
            &grid,
        );

        // Content should be at the right edge of content area
        let expected_x = content_area.right() - 300.0;
        assert!((placement.content_rect.x - expected_x).abs() < 0.01);

        // Right cell (col 1): fold on left, content should be pushed left
        let slot_right = make_slot(0, 1, false);
        let placement = place_page(
            &content_area,
            300.0,
            500.0,
            ScalingMode::None,
            &slot_right,
            &grid,
        );

        // Content should be at the left edge of content area
        assert!((placement.content_rect.x - content_area.x).abs() < 0.01);
    }
}
