//! Grid layout calculation
//!
//! This module handles the geometric layout of the page grid on a sheet,
//! including cell dimensions and fold/cut positions.

use crate::types::PageArrangement;

use super::{GridLayout, GridPosition, Rect};

// =============================================================================
// Grid Creation
// =============================================================================

/// Create a grid layout for the given page arrangement.
///
/// # Arguments
/// * `arrangement` - The page arrangement (folio, quarto, octavo, custom)
/// * `leaf_width_pt` - Width of the leaf area in points (after sheet margins)
/// * `leaf_height_pt` - Height of the leaf area in points (after sheet margins)
/// * `output_width_pt` - Total output sheet width in points
/// * `output_height_pt` - Total output sheet height in points
pub fn create_grid_layout(
    arrangement: PageArrangement,
    leaf_width_pt: f32,
    leaf_height_pt: f32,
    output_width_pt: f32,
    output_height_pt: f32,
) -> GridLayout {
    let (cols, rows) = arrangement.grid_dimensions();

    let cell_width_pt = leaf_width_pt / cols as f32;
    let cell_height_pt = leaf_height_pt / rows as f32;

    let is_landscape = output_width_pt > output_height_pt;

    let FoldCutConfig {
        vertical_folds,
        horizontal_folds,
        vertical_cuts,
        horizontal_spine,
    } = calculate_fold_cut_config(arrangement, is_landscape);

    GridLayout {
        cols,
        rows,
        cell_width_pt,
        cell_height_pt,
        vertical_folds,
        horizontal_folds,
        vertical_cuts,
        horizontal_spine,
    }
}

// =============================================================================
// Fold/Cut Configuration
// =============================================================================

/// Configuration for fold and cut positions
struct FoldCutConfig {
    vertical_folds: Vec<usize>,
    horizontal_folds: Vec<usize>,
    vertical_cuts: Vec<usize>,
    horizontal_spine: bool,
}

/// Calculate fold and cut positions for an arrangement.
fn calculate_fold_cut_config(arrangement: PageArrangement, is_landscape: bool) -> FoldCutConfig {
    match arrangement {
        PageArrangement::Folio => FoldCutConfig {
            // Folio: single vertical fold in the center
            vertical_folds: vec![0],
            horizontal_folds: vec![],
            vertical_cuts: vec![],
            horizontal_spine: false,
        },
        PageArrangement::Quarto => {
            if is_landscape {
                // Landscape quarto: spine is horizontal (between rows)
                FoldCutConfig {
                    vertical_folds: vec![0],
                    horizontal_folds: vec![0],
                    vertical_cuts: vec![],
                    horizontal_spine: true,
                }
            } else {
                // Portrait quarto: spine is vertical (between columns)
                FoldCutConfig {
                    vertical_folds: vec![0],
                    horizontal_folds: vec![0],
                    vertical_cuts: vec![],
                    horizontal_spine: false,
                }
            }
        }
        PageArrangement::Octavo => {
            // Octavo: 4 cols x 2 rows
            // Vertical folds at cols 0 and 2, vertical CUT at col 1 (center)
            // Horizontal fold between rows
            FoldCutConfig {
                vertical_folds: vec![0, 2],
                horizontal_folds: vec![0],
                vertical_cuts: vec![1],
                horizontal_spine: false,
            }
        }
        PageArrangement::Custom { .. } => {
            // Generic: fold between columns
            FoldCutConfig {
                vertical_folds: vec![0],
                horizontal_folds: vec![],
                vertical_cuts: vec![],
                horizontal_spine: false,
            }
        }
    }
}

// =============================================================================
// Cell Calculations
// =============================================================================

/// Calculate the bounds of a cell at the given grid position.
///
/// # Arguments
/// * `grid` - The grid layout
/// * `pos` - Grid position (row, col)
/// * `leaf_origin` - Bottom-left corner of the leaf area (x, y) in points
///
/// # Returns
/// A `Rect` representing the cell bounds.
pub fn cell_bounds(grid: &GridLayout, pos: GridPosition, leaf_origin: (f32, f32)) -> Rect {
    let (leaf_x, leaf_y) = leaf_origin;

    // Calculate cell position
    // Row 0 is at the top, so we need to invert the y calculation
    let cell_x = leaf_x + pos.col as f32 * grid.cell_width_pt;
    let cell_y = leaf_y + (grid.rows - pos.row - 1) as f32 * grid.cell_height_pt;

    Rect::new(cell_x, cell_y, grid.cell_width_pt, grid.cell_height_pt)
}

// =============================================================================
// Edge Information
// =============================================================================

/// Which edges of a cell have folds
#[derive(Debug, Clone, Copy, Default)]
pub struct CellFoldEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl CellFoldEdges {
    /// Check if any edge has a fold
    pub fn any(&self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// Get fold edges for a cell
pub fn cell_fold_edges(grid: &GridLayout, pos: GridPosition) -> CellFoldEdges {
    CellFoldEdges {
        left: grid.has_fold_left(pos.col),
        right: grid.has_fold_right(pos.col),
        top: grid.has_fold_top(pos.row),
        bottom: grid.has_fold_bottom(pos.row),
    }
}

/// Complete edge information for a cell
///
/// This provides all the information needed to determine margins and
/// printer's marks for a cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct CellEdgeInfo {
    // Folds
    pub fold_left: bool,
    pub fold_right: bool,
    pub fold_top: bool,
    pub fold_bottom: bool,

    // Cuts
    pub cut_left: bool,
    pub cut_right: bool,
    pub cut_top: bool,
    pub cut_bottom: bool,

    // Outer edges (sheet boundary)
    pub outer_left: bool,
    pub outer_right: bool,
    pub outer_top: bool,
    pub outer_bottom: bool,

    // Spine orientation
    pub horizontal_spine: bool,
}

impl CellEdgeInfo {
    /// Returns true if the left edge is a spine fold
    pub fn is_spine_left(&self) -> bool {
        self.fold_left && !self.horizontal_spine
    }

    /// Returns true if the right edge is a spine fold
    pub fn is_spine_right(&self) -> bool {
        self.fold_right && !self.horizontal_spine
    }

    /// Returns true if the top edge is a spine fold
    pub fn is_spine_top(&self) -> bool {
        self.fold_top && self.horizontal_spine
    }

    /// Returns true if the bottom edge is a spine fold
    pub fn is_spine_bottom(&self) -> bool {
        self.fold_bottom && self.horizontal_spine
    }
}

/// Get complete edge information for a cell
pub fn cell_edge_info(grid: &GridLayout, pos: GridPosition) -> CellEdgeInfo {
    CellEdgeInfo {
        fold_left: grid.has_fold_left(pos.col),
        fold_right: grid.has_fold_right(pos.col),
        fold_top: grid.has_fold_top(pos.row),
        fold_bottom: grid.has_fold_bottom(pos.row),

        cut_left: grid.has_cut_left(pos.col),
        cut_right: grid.has_cut_right(pos.col),
        cut_top: false, // No horizontal cuts currently supported
        cut_bottom: false,

        outer_left: grid.is_outer_left(pos.col),
        outer_right: grid.is_outer_right(pos.col),
        outer_top: grid.is_outer_top(pos.row),
        outer_bottom: grid.is_outer_bottom(pos.row),

        horizontal_spine: grid.horizontal_spine,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_folio_grid() {
        let grid = create_grid_layout(PageArrangement::Folio, 800.0, 600.0, 850.0, 650.0);

        assert_eq!(grid.cols, 2);
        assert_eq!(grid.rows, 1);
        assert_eq!(grid.cell_width_pt, 400.0);
        assert_eq!(grid.cell_height_pt, 600.0);
        assert_eq!(grid.vertical_folds, vec![0]);
        assert!(grid.horizontal_folds.is_empty());
    }

    #[test]
    fn test_quarto_grid() {
        let grid = create_grid_layout(PageArrangement::Quarto, 800.0, 600.0, 850.0, 650.0);

        assert_eq!(grid.cols, 2);
        assert_eq!(grid.rows, 2);
        assert_eq!(grid.cell_width_pt, 400.0);
        assert_eq!(grid.cell_height_pt, 300.0);
    }

    #[test]
    fn test_octavo_grid() {
        let grid = create_grid_layout(PageArrangement::Octavo, 800.0, 600.0, 850.0, 650.0);

        assert_eq!(grid.cols, 4);
        assert_eq!(grid.rows, 2);
        assert_eq!(grid.cell_width_pt, 200.0);
        assert_eq!(grid.cell_height_pt, 300.0);
        // Folds at cols 0 and 2, cut at col 1
        assert_eq!(grid.vertical_folds, vec![0, 2]);
        assert_eq!(grid.vertical_cuts, vec![1]);
    }

    #[test]
    fn test_cell_bounds() {
        let grid = create_grid_layout(PageArrangement::Quarto, 800.0, 600.0, 850.0, 650.0);

        // Bottom-left cell (row 1, col 0)
        let bounds = cell_bounds(&grid, GridPosition::new(1, 0), (25.0, 25.0));
        assert_eq!(bounds.x, 25.0);
        assert_eq!(bounds.y, 25.0);
        assert_eq!(bounds.width, 400.0);
        assert_eq!(bounds.height, 300.0);

        // Top-right cell (row 0, col 1)
        let bounds = cell_bounds(&grid, GridPosition::new(0, 1), (25.0, 25.0));
        assert_eq!(bounds.x, 425.0);
        assert_eq!(bounds.y, 325.0);
    }

    #[test]
    fn test_cell_fold_edges() {
        let grid = create_grid_layout(PageArrangement::Quarto, 800.0, 600.0, 850.0, 650.0);

        // Top-left cell (row 0, col 0): fold on right and bottom
        let edges = cell_fold_edges(&grid, GridPosition::new(0, 0));
        assert!(!edges.left);
        assert!(edges.right);
        assert!(!edges.top);
        assert!(edges.bottom);

        // Top-right cell (row 0, col 1): fold on left and bottom
        let edges = cell_fold_edges(&grid, GridPosition::new(0, 1));
        assert!(edges.left);
        assert!(!edges.right);
        assert!(!edges.top);
        assert!(edges.bottom);

        // Bottom-right cell (row 1, col 1): fold on left and top
        let edges = cell_fold_edges(&grid, GridPosition::new(1, 1));
        assert!(edges.left);
        assert!(!edges.right);
        assert!(edges.top);
        assert!(!edges.bottom);
    }

    #[test]
    fn test_cell_edge_info_outer_edges() {
        let grid = create_grid_layout(PageArrangement::Quarto, 800.0, 600.0, 850.0, 650.0);

        // Top-left is outer top and left
        let info = cell_edge_info(&grid, GridPosition::new(0, 0));
        assert!(info.outer_top);
        assert!(info.outer_left);
        assert!(!info.outer_right);
        assert!(!info.outer_bottom);

        // Bottom-right is outer bottom and right
        let info = cell_edge_info(&grid, GridPosition::new(1, 1));
        assert!(!info.outer_top);
        assert!(!info.outer_left);
        assert!(info.outer_right);
        assert!(info.outer_bottom);
    }
}
