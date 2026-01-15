//! Grid layout calculation
//!
//! This module handles the geometric layout of the page grid on a sheet,
//! including cell dimensions and fold/cut positions.

use crate::types::PageArrangement;

use super::{GridLayout, GridPosition, Rect};

/// Create a grid layout for the given page arrangement.
///
/// # Arguments
/// * `arrangement` - The page arrangement (folio, quarto, octavo, custom)
/// * `leaf_width_pt` - Width of the leaf area in points (after sheet margins)
/// * `leaf_height_pt` - Height of the leaf area in points (after sheet margins)
/// * `output_width_pt` - Total output sheet width in points
/// * `output_height_pt` - Total output sheet height in points
///
/// # Returns
/// A `GridLayout` describing the cell dimensions and fold/cut positions.
pub fn create_grid_layout(
    arrangement: PageArrangement,
    leaf_width_pt: f32,
    leaf_height_pt: f32,
    output_width_pt: f32,
    output_height_pt: f32,
) -> GridLayout {
    let (cols, rows) = grid_dimensions(arrangement);

    let cell_width_pt = leaf_width_pt / cols as f32;
    let cell_height_pt = leaf_height_pt / rows as f32;

    let is_landscape = output_width_pt > output_height_pt;

    let (vertical_folds, horizontal_folds, vertical_cuts, horizontal_spine) =
        fold_positions(arrangement, is_landscape);

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

/// Get grid dimensions (cols, rows) for an arrangement
pub fn grid_dimensions(arrangement: PageArrangement) -> (usize, usize) {
    match arrangement {
        PageArrangement::Folio => (2, 1),
        PageArrangement::Quarto => (2, 2),
        PageArrangement::Octavo => (4, 2),
        PageArrangement::Custom {
            pages_per_signature,
        } => {
            // For custom arrangements, use 2-up layout
            let pages_per_side = pages_per_signature / 2;
            if pages_per_side <= 2 {
                (2, 1)
            } else if pages_per_side <= 4 {
                (2, 2)
            } else {
                (4, (pages_per_side + 3) / 4)
            }
        }
    }
}

/// Calculate fold and cut positions for an arrangement.
///
/// Returns (vertical_folds, horizontal_folds, vertical_cuts, horizontal_spine)
fn fold_positions(
    arrangement: PageArrangement,
    is_landscape: bool,
) -> (Vec<usize>, Vec<usize>, Vec<usize>, bool) {
    match arrangement {
        PageArrangement::Folio => {
            // Folio: single vertical fold in the center
            (vec![0], vec![], vec![], false)
        }
        PageArrangement::Quarto => {
            if is_landscape {
                // Landscape quarto: spine is horizontal (between rows)
                // Fold between rows, fold between columns
                (vec![0], vec![0], vec![], true)
            } else {
                // Portrait quarto: spine is vertical (between columns)
                // Fold between rows, fold between columns
                (vec![0], vec![0], vec![], false)
            }
        }
        PageArrangement::Octavo => {
            // Octavo: 4 cols x 2 rows
            // Vertical folds at cols 0 and 2, vertical CUT at col 1 (center)
            // Horizontal fold between rows
            (vec![0, 2], vec![0], vec![1], false)
        }
        PageArrangement::Custom { .. } => {
            // Generic: fold between columns
            (vec![0], vec![], vec![], false)
        }
    }
}

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

/// Check if a cell has a fold on a specific edge.
pub fn cell_fold_edges(grid: &GridLayout, pos: GridPosition) -> CellFoldEdges {
    CellFoldEdges {
        left: pos.col > 0 && grid.vertical_folds.contains(&(pos.col - 1)),
        right: grid.vertical_folds.contains(&pos.col),
        top: pos.row > 0 && grid.horizontal_folds.contains(&(pos.row - 1)),
        bottom: grid.horizontal_folds.contains(&pos.row),
    }
}

/// Which edges of a cell are adjacent to folds
#[derive(Debug, Clone, Copy, Default)]
pub struct CellFoldEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl CellFoldEdges {
    /// Check if any edge has a fold
    pub fn has_any(&self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

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
}
