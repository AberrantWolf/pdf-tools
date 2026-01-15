//! Content placement within cells
//!
//! This module handles calculating the final position of page content
//! within grid cells, accounting for:
//! - Leaf margins (spine vs fore-edge)
//! - Page rotation
//! - Content alignment toward folds
//! - Scaling

use crate::types::{LeafMargins, ScalingMode};

use super::{GridLayout, PagePlacement, Rect, SignatureSlot, cell_bounds, cell_fold_edges};

/// Calculate the content area within a cell, accounting for margins.
///
/// The content area is the region where page content can be placed,
/// after applying leaf margins. Margins are applied based on:
/// - Which edge is the spine (determined by page side and fold positions)
/// - Whether the page is rotated (which swaps left/right margins)
///
/// # Arguments
/// * `cell` - The cell bounds
/// * `margins` - Leaf margins configuration
/// * `slot` - The signature slot (contains rotation and page side info)
/// * `grid` - The grid layout (for determining fold positions)
pub fn calculate_content_area(
    cell: &Rect,
    margins: &LeafMargins,
    slot: &SignatureSlot,
    grid: &GridLayout,
) -> Rect {
    let fold_edges = cell_fold_edges(grid, slot.grid_pos);

    // Determine horizontal margins based on fold positions and rotation
    // The spine margin goes on the edge adjacent to a fold
    // The fore-edge margin goes on the outer edge
    let (margin_left, margin_right) = if grid.horizontal_spine {
        // Landscape quarto: spine is horizontal, so left/right are fore-edges
        (margins.fore_edge_mm, margins.fore_edge_mm)
    } else {
        // Vertical spine (normal case)
        let (base_left, base_right) = if fold_edges.right && !fold_edges.left {
            // Fold on right: spine on right, fore-edge on left
            (margins.fore_edge_mm, margins.spine_mm)
        } else if fold_edges.left && !fold_edges.right {
            // Fold on left: spine on left, fore-edge on right
            (margins.spine_mm, margins.fore_edge_mm)
        } else if fold_edges.left && fold_edges.right {
            // Folds on both sides (octavo inner columns): spine on both
            (margins.spine_mm, margins.spine_mm)
        } else {
            // No horizontal folds: use average
            let avg = (margins.fore_edge_mm + margins.spine_mm) / 2.0;
            (avg, avg)
        };

        // For rotated pages, left/right swap because the page is upside down
        if slot.rotated {
            (base_right, base_left)
        } else {
            (base_left, base_right)
        }
    };

    // Determine vertical margins based on fold positions
    let (margin_bottom, margin_top) = if grid.horizontal_spine {
        // Landscape quarto: spine is horizontal between rows
        if fold_edges.bottom {
            // Top row: fold at bottom = spine at bottom
            (margins.spine_mm, margins.fore_edge_mm)
        } else if fold_edges.top {
            // Bottom row: fold at top = spine at top
            (margins.fore_edge_mm, margins.spine_mm)
        } else {
            (margins.bottom_mm, margins.top_mm)
        }
    } else if slot.rotated {
        // Portrait with vertical spine: page rotated 180°, so top becomes bottom
        (margins.top_mm, margins.bottom_mm)
    } else {
        (margins.bottom_mm, margins.top_mm)
    };

    // Convert margins from mm to points
    let left_pt = mm_to_pt(margin_left);
    let right_pt = mm_to_pt(margin_right);
    let bottom_pt = mm_to_pt(margin_bottom);
    let top_pt = mm_to_pt(margin_top);

    Rect::new(
        cell.x + left_pt,
        cell.y + bottom_pt,
        cell.width - left_pt - right_pt,
        cell.height - bottom_pt - top_pt,
    )
}

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
    // Content should be pushed toward folds (where pages meet after folding)
    let fold_edges = cell_fold_edges(grid, slot.grid_pos);

    // Horizontal alignment
    let x = if fold_edges.right && !fold_edges.left {
        // Fold on right: push content right
        content_area.x + content_area.width - scaled_width
    } else if fold_edges.left && !fold_edges.right {
        // Fold on left: push content left
        content_area.x
    } else {
        // No preference or both sides: center
        content_area.x + (content_area.width - scaled_width) / 2.0
    };

    // Vertical alignment
    let y = if fold_edges.bottom && !fold_edges.top {
        // Fold at bottom: push content down
        content_area.y
    } else if fold_edges.top && !fold_edges.bottom {
        // Fold at top: push content up
        content_area.y + content_area.height - scaled_height
    } else {
        // No preference or both: center
        content_area.y + (content_area.height - scaled_height) / 2.0
    };

    let rotation_degrees = if slot.rotated { 180.0 } else { 0.0 };

    PagePlacement {
        source_page: None, // Will be filled in by caller
        content_rect: Rect::new(x, y, scaled_width, scaled_height),
        rotation_degrees,
        scale,
        slot: slot.clone(),
    }
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
                .unwrap_or((612.0, 792.0)); // Default to US Letter

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

/// Calculate scale factor for fitting source to target dimensions.
fn calculate_scale(
    src_width: f32,
    src_height: f32,
    target_width: f32,
    target_height: f32,
    mode: ScalingMode,
) -> f32 {
    match mode {
        ScalingMode::Fit => {
            let scale_w = target_width / src_width;
            let scale_h = target_height / src_height;
            scale_w.min(scale_h)
        }
        ScalingMode::Fill => {
            let scale_w = target_width / src_width;
            let scale_h = target_height / src_height;
            scale_w.max(scale_h)
        }
        ScalingMode::None => 1.0,
        ScalingMode::Stretch => {
            // Use width scaling (aspect ratio ignored in one dimension)
            target_width / src_width
        }
    }
}

/// Convert millimeters to points
fn mm_to_pt(mm: f32) -> f32 {
    mm * 2.83465
}

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

    #[test]
    fn test_content_area_with_margins() {
        let cell = Rect::new(0.0, 0.0, 400.0, 600.0);
        let margins = LeafMargins {
            top_mm: 5.0,
            bottom_mm: 5.0,
            fore_edge_mm: 5.0,
            spine_mm: 10.0,
        };

        let grid =
            super::super::create_grid_layout(PageArrangement::Folio, 800.0, 600.0, 850.0, 650.0);

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
    fn test_rotated_margins_swap() {
        let cell = Rect::new(0.0, 0.0, 400.0, 600.0);
        let margins = LeafMargins {
            top_mm: 5.0,
            bottom_mm: 5.0,
            fore_edge_mm: 5.0,
            spine_mm: 10.0,
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

        // The x position should differ because left/right margins swap
        // fore_edge = 5mm ≈ 14.17pt, spine = 10mm ≈ 28.35pt
        // Difference should be about 14pt
        assert!(
            (area_normal.x - area_rotated.x).abs() > 10.0,
            "Expected margin swap: normal.x={}, rotated.x={}",
            area_normal.x,
            area_rotated.x
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
    fn test_alignment_toward_fold() {
        let content_area = Rect::new(10.0, 10.0, 400.0, 600.0);
        let grid =
            super::super::create_grid_layout(PageArrangement::Folio, 800.0, 600.0, 850.0, 650.0);

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
        let expected_x = content_area.x + content_area.width - 300.0;
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
