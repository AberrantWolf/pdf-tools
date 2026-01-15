//! Layout data types for imposition
//!
//! These types represent the intermediate layout calculations between
//! signature ordering and PDF rendering:
//! - SignatureSlot: Where a page goes in the signature
//! - GridLayout: The cell arrangement on a sheet
//! - PagePlacement: Final rendering information for a page
//! - SheetLayout: All placements for one side of a sheet

// =============================================================================
// Page and Sheet Sides
// =============================================================================

/// Which side of a bound book this page appears on after folding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageSide {
    /// Right-hand page (odd page numbers in final book: 1, 3, 5, ...)
    /// The spine edge is on the left
    #[default]
    Recto,
    /// Left-hand page (even page numbers in final book: 2, 4, 6, ...)
    /// The spine edge is on the right
    Verso,
}

impl PageSide {
    /// Returns true if this is a recto (right-hand) page
    pub fn is_recto(self) -> bool {
        matches!(self, PageSide::Recto)
    }

    /// Returns the opposite side
    pub fn opposite(self) -> Self {
        match self {
            PageSide::Recto => PageSide::Verso,
            PageSide::Verso => PageSide::Recto,
        }
    }

    /// Get page side from 1-based page number
    pub fn from_page_number(page_num: usize) -> Self {
        if page_num % 2 == 1 {
            PageSide::Recto // Odd pages are recto
        } else {
            PageSide::Verso // Even pages are verso
        }
    }
}

/// Which physical side of the printed sheet
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SheetSide {
    /// Front of the sheet (printed first in duplex)
    #[default]
    Front,
    /// Back of the sheet (printed second in duplex)
    Back,
}

impl SheetSide {
    /// Returns true if this is the front side
    pub fn is_front(self) -> bool {
        matches!(self, SheetSide::Front)
    }

    /// Returns the opposite side
    pub fn opposite(self) -> Self {
        match self {
            SheetSide::Front => SheetSide::Back,
            SheetSide::Back => SheetSide::Front,
        }
    }
}

// =============================================================================
// Grid Position
// =============================================================================

/// Position within the grid (row, column)
///
/// Row 0 is the top row, column 0 is the leftmost column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct GridPosition {
    /// Row index (0 = top row)
    pub row: usize,
    /// Column index (0 = leftmost column)
    pub col: usize,
}

impl GridPosition {
    /// Create a new grid position
    pub const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    /// Convert to flat index in row-major order
    pub fn to_index(self, cols: usize) -> usize {
        self.row * cols + self.col
    }

    /// Create from flat index in row-major order
    pub fn from_index(index: usize, cols: usize) -> Self {
        Self {
            row: index / cols,
            col: index % cols,
        }
    }
}

// =============================================================================
// Signature Slot
// =============================================================================

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

impl SignatureSlot {
    /// Create a new signature slot
    pub fn new(
        slot_index: usize,
        sheet_side: SheetSide,
        row: usize,
        col: usize,
        rotated: bool,
        page_side: PageSide,
    ) -> Self {
        Self {
            slot_index,
            sheet_side,
            grid_pos: GridPosition::new(row, col),
            rotated,
            page_side,
        }
    }

    /// Get rotation in degrees (0 or 180)
    pub fn rotation_degrees(&self) -> f32 {
        if self.rotated { 180.0 } else { 0.0 }
    }
}

// =============================================================================
// Grid Layout
// =============================================================================

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

    /// Check if a column has a cut on its right edge
    pub fn has_cut_right(&self, col: usize) -> bool {
        self.vertical_cuts.contains(&col)
    }

    /// Check if a column has a cut on its left edge
    pub fn has_cut_left(&self, col: usize) -> bool {
        col > 0 && self.vertical_cuts.contains(&(col - 1))
    }

    /// Total number of cells in the grid
    pub fn cell_count(&self) -> usize {
        self.cols * self.rows
    }

    /// Check if a position is on the outer left edge
    pub fn is_outer_left(&self, col: usize) -> bool {
        col == 0
    }

    /// Check if a position is on the outer right edge
    pub fn is_outer_right(&self, col: usize) -> bool {
        col == self.cols - 1
    }

    /// Check if a position is on the outer top edge
    pub fn is_outer_top(&self, row: usize) -> bool {
        row == 0
    }

    /// Check if a position is on the outer bottom edge
    pub fn is_outer_bottom(&self, row: usize) -> bool {
        row == self.rows - 1
    }
}

// =============================================================================
// Rectangle
// =============================================================================

/// A rectangular area in points
///
/// Used for cell bounds, content areas, and page placements.
/// Coordinates are in PDF space (origin at bottom-left).
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
    /// Create a new rectangle
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create from corner points (left, bottom, right, top)
    pub fn from_corners(left: f32, bottom: f32, right: f32, top: f32) -> Self {
        Self {
            x: left,
            y: bottom,
            width: right - left,
            height: top - bottom,
        }
    }

    /// Left edge x coordinate (same as x)
    pub fn left(&self) -> f32 {
        self.x
    }

    /// Bottom edge y coordinate (same as y)
    pub fn bottom(&self) -> f32 {
        self.y
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

    /// Center point as (x, y) tuple
    pub fn center(&self) -> (f32, f32) {
        (self.center_x(), self.center_y())
    }

    /// Area of the rectangle
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Check if the rectangle has positive area
    pub fn is_valid(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }

    /// Inset the rectangle by the given amounts
    pub fn inset(&self, left: f32, bottom: f32, right: f32, top: f32) -> Self {
        Self {
            x: self.x + left,
            y: self.y + bottom,
            width: self.width - left - right,
            height: self.height - bottom - top,
        }
    }

    /// Inset the rectangle uniformly on all sides
    pub fn inset_uniform(&self, amount: f32) -> Self {
        self.inset(amount, amount, amount, amount)
    }
}

// =============================================================================
// Page Placement
// =============================================================================

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

impl PagePlacement {
    /// Returns true if this is a blank page
    pub fn is_blank(&self) -> bool {
        self.source_page.is_none()
    }

    /// Returns true if the page is rotated
    pub fn is_rotated(&self) -> bool {
        self.rotation_degrees.abs() > 0.1
    }
}

// =============================================================================
// Sheet Layout
// =============================================================================

/// Information about a single output sheet side
///
/// Contains all the page placements and bounds for rendering one side
/// of a physical sheet.
#[derive(Debug, Clone)]
pub struct SheetLayout {
    /// Which side of the physical sheet
    pub side: SheetSide,
    /// All page placements for this side
    pub placements: Vec<PagePlacement>,
    /// The leaf area bounds (inside sheet margins)
    pub leaf_bounds: Rect,
}

impl SheetLayout {
    /// Get placements that have actual source pages (not blank)
    pub fn non_blank_placements(&self) -> impl Iterator<Item = &PagePlacement> {
        self.placements.iter().filter(|p| !p.is_blank())
    }

    /// Number of non-blank pages on this sheet side
    pub fn content_count(&self) -> usize {
        self.placements.iter().filter(|p| !p.is_blank()).count()
    }
}
