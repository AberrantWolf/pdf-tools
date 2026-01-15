//! Layout data types for imposition
//!
//! These types represent the intermediate layout calculations between
//! signature ordering and PDF rendering.

/// Which side of a bound book this page appears on after folding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageSide {
    /// Right-hand page (odd page numbers in final book)
    /// The spine edge is on the left
    Recto,
    /// Left-hand page (even page numbers in final book)
    /// The spine edge is on the right
    Verso,
}

/// Which physical side of the printed sheet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetSide {
    /// Front of the sheet (printed first in duplex)
    Front,
    /// Back of the sheet (printed second in duplex)
    Back,
}

/// A page's position within a signature
///
/// This captures all the information needed to place a page correctly:
/// - Where it goes in the grid
/// - Whether it needs rotation
/// - Which side of the book it will be on after folding
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureSlot {
    /// Index in the flat signature order (0..pages_per_sig)
    pub slot_index: usize,
    /// Which sheet side (front/back)
    pub sheet_side: SheetSide,
    /// Position in grid (row, col) - row 0 is top
    pub grid_pos: GridPosition,
    /// Whether this slot needs 180° rotation
    pub rotated: bool,
    /// Which book side this page will be on after folding
    pub page_side: PageSide,
}

/// Position within the grid (row, column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPosition {
    /// Row index (0 = top row)
    pub row: usize,
    /// Column index (0 = leftmost column)
    pub col: usize,
}

impl GridPosition {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

/// Grid layout for a folding scheme
///
/// Describes the physical layout of pages on a sheet, including
/// where folds and cuts occur.
#[derive(Debug, Clone, PartialEq)]
pub struct GridLayout {
    /// Number of columns in the page grid
    pub cols: usize,
    /// Number of rows in the page grid
    pub rows: usize,
    /// Width of each cell in points
    pub cell_width_pt: f32,
    /// Height of each cell in points
    pub cell_height_pt: f32,
    /// Column indices that have a fold on their right edge
    /// (e.g., for 2 cols: [0] means fold between col 0 and col 1)
    pub vertical_folds: Vec<usize>,
    /// Row indices that have a fold on their bottom edge
    /// (e.g., for 2 rows: [0] means fold between row 0 and row 1)
    pub horizontal_folds: Vec<usize>,
    /// Column indices where vertical cuts occur
    /// (used in octavo where center is cut, not folded)
    pub vertical_cuts: Vec<usize>,
    /// Whether the spine runs horizontally (true for landscape quarto)
    pub horizontal_spine: bool,
}

impl GridLayout {
    /// Check if a column has a fold on its right edge
    pub fn has_fold_right(&self, col: usize) -> bool {
        self.vertical_folds.contains(&col)
    }

    /// Check if a column has a fold on its left edge
    pub fn has_fold_left(&self, col: usize) -> bool {
        col > 0 && self.vertical_folds.contains(&(col - 1))
    }

    /// Check if a row has a fold on its bottom edge
    pub fn has_fold_bottom(&self, row: usize) -> bool {
        self.horizontal_folds.contains(&row)
    }

    /// Check if a row has a fold on its top edge
    pub fn has_fold_top(&self, row: usize) -> bool {
        row > 0 && self.horizontal_folds.contains(&(row - 1))
    }

    /// Total number of cells in the grid
    pub fn cell_count(&self) -> usize {
        self.cols * self.rows
    }
}

/// A rectangular area in points
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// X position (left edge)
    pub x: f32,
    /// Y position (bottom edge)
    pub y: f32,
    /// Width
    pub width: f32,
    /// Height
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge x coordinate
    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    /// Top edge y coordinate
    pub fn top(&self) -> f32 {
        self.y + self.height
    }

    /// Center x coordinate
    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }

    /// Center y coordinate
    pub fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }
}

/// Final placement of a source page on the output sheet
///
/// This is the result of all layout calculations and contains
/// everything needed to render the page.
#[derive(Debug, Clone, PartialEq)]
pub struct PagePlacement {
    /// Source page index (None = blank page)
    pub source_page: Option<usize>,
    /// Position and size of the page content in points
    pub content_rect: Rect,
    /// Rotation to apply in degrees (0.0 or 180.0)
    pub rotation_degrees: f32,
    /// Scale factor applied to the source page
    pub scale: f32,
    /// The signature slot this placement corresponds to
    pub slot: SignatureSlot,
}

/// Information about a single output sheet side
#[derive(Debug, Clone)]
pub struct SheetLayout {
    /// Which side of the physical sheet
    pub side: SheetSide,
    /// All page placements for this side
    pub placements: Vec<PagePlacement>,
    /// The leaf area bounds (inside sheet margins)
    pub leaf_bounds: Rect,
}
