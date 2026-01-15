//! Core types for PDF imposition
//!
//! This module defines the fundamental types used throughout the imposition process:
//! - Error types and Result alias
//! - Paper sizes and orientation
//! - Binding and arrangement options
//! - Margin configurations
//! - Printer's marks settings

use thiserror::Error;

// =============================================================================
// Error Handling
// =============================================================================

/// Errors that can occur during imposition
#[derive(Error, Debug)]
pub enum ImposeError {
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid configuration: {0}")]
    Config(String),

    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("No pages to impose")]
    NoPages,
}

/// Result type alias for imposition operations
pub type Result<T> = std::result::Result<T, ImposeError>;

// =============================================================================
// Paper Configuration
// =============================================================================

/// Paper orientation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    /// Portrait: height > width (default for most paper sizes)
    #[default]
    Portrait,
    /// Landscape: width > height
    Landscape,
}

impl Orientation {
    /// Returns true if landscape orientation
    pub fn is_landscape(self) -> bool {
        matches!(self, Orientation::Landscape)
    }

    /// Returns the opposite orientation
    pub fn flip(self) -> Self {
        match self {
            Orientation::Portrait => Orientation::Landscape,
            Orientation::Landscape => Orientation::Portrait,
        }
    }
}

/// Standard paper sizes
///
/// All dimensions are stored in portrait orientation (width < height).
/// Use `dimensions_with_orientation` to get landscape dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperSize {
    /// ISO A3 (297mm × 420mm)
    A3,
    /// ISO A4 (210mm × 297mm)
    A4,
    /// ISO A5 (148mm × 210mm)
    A5,
    /// US Letter (8.5" × 11")
    Letter,
    /// US Legal (8.5" × 14")
    Legal,
    /// US Tabloid (11" × 17")
    Tabloid,
    /// Custom dimensions in millimeters
    Custom { width_mm: f32, height_mm: f32 },
}

impl Default for PaperSize {
    fn default() -> Self {
        PaperSize::Letter
    }
}

impl PaperSize {
    /// Get base dimensions in millimeters (always portrait: width < height for standard sizes)
    pub fn dimensions_mm(self) -> (f32, f32) {
        match self {
            PaperSize::A3 => (297.0, 420.0),
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::Legal => (215.9, 355.6),
            PaperSize::Tabloid => (279.4, 431.8),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
        }
    }

    /// Get dimensions with orientation applied
    pub fn dimensions_with_orientation(self, orientation: Orientation) -> (f32, f32) {
        let (w, h) = self.dimensions_mm();
        match orientation {
            Orientation::Portrait => (w, h),
            Orientation::Landscape => (h, w),
        }
    }

    /// Get dimensions in points (1/72 inch)
    pub fn dimensions_pt(self) -> (f32, f32) {
        let (w, h) = self.dimensions_mm();
        (crate::constants::mm_to_pt(w), crate::constants::mm_to_pt(h))
    }

    /// Get dimensions in points with orientation applied
    pub fn dimensions_pt_with_orientation(self, orientation: Orientation) -> (f32, f32) {
        let (w, h) = self.dimensions_with_orientation(orientation);
        (crate::constants::mm_to_pt(w), crate::constants::mm_to_pt(h))
    }
}

// =============================================================================
// Binding Configuration
// =============================================================================

/// Binding methods for the finished book
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum BindingType {
    /// Saddle-stitch binding (folded sheets, stapled at spine)
    /// Best for booklets up to ~64 pages
    #[default]
    Signature,
    /// Perfect binding (pages glued to spine)
    /// Best for thicker books
    PerfectBinding,
    /// Side stitch binding (staples through side margin)
    SideStitch,
    /// Spiral/coil binding
    Spiral,
    /// Case binding (sewn signatures in hardcover)
    CaseBinding,
}

impl BindingType {
    /// Returns true if this binding type uses signatures (folded sheets)
    pub fn uses_signatures(self) -> bool {
        matches!(self, BindingType::Signature | BindingType::CaseBinding)
    }
}

/// Page arrangement within a signature
///
/// Determines how many pages fit on each sheet and how they're folded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageArrangement {
    /// Folio: 4 pages per sheet (1 fold)
    /// Grid: 2 columns × 1 row
    Folio,
    /// Quarto: 8 pages per sheet (2 folds)
    /// Grid: 2 columns × 2 rows
    #[default]
    Quarto,
    /// Octavo: 16 pages per sheet (3 folds)
    /// Grid: 4 columns × 2 rows
    Octavo,
    /// Custom pages per signature (must be multiple of 4)
    Custom { pages_per_signature: usize },
}

impl PageArrangement {
    /// Number of pages per signature
    pub fn pages_per_signature(self) -> usize {
        match self {
            PageArrangement::Folio => 4,
            PageArrangement::Quarto => 8,
            PageArrangement::Octavo => 16,
            PageArrangement::Custom {
                pages_per_signature,
            } => pages_per_signature,
        }
    }

    /// Number of sheets per signature
    pub fn sheets_per_signature(self) -> usize {
        self.pages_per_signature() / 4
    }

    /// Grid dimensions (columns, rows) for this arrangement
    pub fn grid_dimensions(self) -> (usize, usize) {
        match self {
            PageArrangement::Folio => (2, 1),
            PageArrangement::Quarto => (2, 2),
            PageArrangement::Octavo => (4, 2),
            PageArrangement::Custom {
                pages_per_signature,
            } => {
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
}

// =============================================================================
// Output Configuration
// =============================================================================

/// Output PDF format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum OutputFormat {
    /// Single PDF with both sides interleaved (page 1 front, page 1 back, page 2 front, ...)
    #[default]
    DoubleSided,
    /// Two separate PDFs (all fronts, then all backs)
    TwoSided,
    /// Single PDF with pages in print order for single-sided printing
    SingleSidedSequence,
}

/// Page scaling behavior when source pages don't match output cell size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ScalingMode {
    /// Fit entire page within available space (preserve aspect ratio, may have margins)
    #[default]
    Fit,
    /// Fill available space (preserve aspect ratio, may crop)
    Fill,
    /// No scaling (center at original size)
    None,
    /// Stretch to fill (ignore aspect ratio)
    Stretch,
}

/// Rotation to apply to source pages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Rotation {
    #[default]
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl Rotation {
    /// Get rotation in degrees
    pub fn degrees(self) -> i32 {
        match self {
            Rotation::None => 0,
            Rotation::Clockwise90 => 90,
            Rotation::Clockwise180 => 180,
            Rotation::Clockwise270 => 270,
        }
    }

    /// Create from degrees (normalized to 0, 90, 180, 270)
    pub fn from_degrees(deg: i32) -> Self {
        match deg.rem_euclid(360) {
            0 => Rotation::None,
            90 => Rotation::Clockwise90,
            180 => Rotation::Clockwise180,
            270 => Rotation::Clockwise270,
            _ => Rotation::None, // Snap to nearest 90°
        }
    }
}

// =============================================================================
// Margins
// =============================================================================

/// Sheet margins - printer-safe area around the entire output sheet.
///
/// These margins ensure content stays within the printer's printable area.
/// Typical home printers need 5-10mm margins; commercial printers may print borderless.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SheetMargins {
    pub top_mm: f32,
    pub bottom_mm: f32,
    pub left_mm: f32,
    pub right_mm: f32,
}

impl Default for SheetMargins {
    fn default() -> Self {
        Self::uniform(5.0)
    }
}

impl SheetMargins {
    /// Create uniform margins on all sides
    pub fn uniform(margin_mm: f32) -> Self {
        Self {
            top_mm: margin_mm,
            bottom_mm: margin_mm,
            left_mm: margin_mm,
            right_mm: margin_mm,
        }
    }

    /// Create with no margins (borderless)
    pub fn none() -> Self {
        Self::uniform(0.0)
    }

    /// Total horizontal margin (left + right)
    pub fn horizontal_mm(&self) -> f32 {
        self.left_mm + self.right_mm
    }

    /// Total vertical margin (top + bottom)
    pub fn vertical_mm(&self) -> f32 {
        self.top_mm + self.bottom_mm
    }
}

/// Leaf margins - applied to each logical page within the imposed sheet.
///
/// These provide:
/// - Trim space for cutting after folding
/// - Spine gutter for readability when bound
/// - Consistent page margins in the final book
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LeafMargins {
    /// Top margin (head) of each leaf
    pub top_mm: f32,
    /// Bottom margin (tail) of each leaf
    pub bottom_mm: f32,
    /// Outer margin (fore edge) - the edge opposite the spine
    pub fore_edge_mm: f32,
    /// Inner margin (spine/gutter) - extra space near the binding
    pub spine_mm: f32,
    /// Margin around cut lines - space between pages that will be cut apart
    pub cut_mm: f32,
}

impl LeafMargins {
    /// Create uniform margins (except spine and cut)
    pub fn uniform(margin_mm: f32) -> Self {
        Self {
            top_mm: margin_mm,
            bottom_mm: margin_mm,
            fore_edge_mm: margin_mm,
            spine_mm: margin_mm,
            cut_mm: 0.0,
        }
    }
}

/// Combined margins for imposition
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Margins {
    /// Printer-safe margins around the entire output sheet
    pub sheet: SheetMargins,
    /// Margins for each logical page/leaf
    pub leaf: LeafMargins,
}

// =============================================================================
// Printer's Marks
// =============================================================================

/// Printer's marks configuration
///
/// These marks help with alignment, folding, and trimming during finishing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrinterMarks {
    /// Add fold lines (dashed) - where paper should be folded
    pub fold_lines: bool,
    /// Add cut lines (solid with scissors) - where paper should be cut after folding
    pub cut_lines: bool,
    /// Add crop marks (L-shaped corner marks at sheet edges)
    pub crop_marks: bool,
    /// Add trim marks (L-shaped corner marks at each page boundary)
    pub trim_marks: bool,
    /// Add registration marks (crosshairs for alignment)
    pub registration_marks: bool,
}

impl PrinterMarks {
    /// Enable all marks
    pub fn all() -> Self {
        Self {
            fold_lines: true,
            cut_lines: true,
            crop_marks: true,
            trim_marks: true,
            registration_marks: true,
        }
    }

    /// Check if any marks are enabled
    pub fn any_enabled(&self) -> bool {
        self.fold_lines
            || self.cut_lines
            || self.crop_marks
            || self.trim_marks
            || self.registration_marks
    }
}

// =============================================================================
// Output Splitting
// =============================================================================

/// How to split the output PDF
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum SplitMode {
    /// No splitting - single output file
    #[default]
    None,
    /// Split by number of pages
    ByPages(usize),
    /// Split by number of sheets
    BySheets(usize),
    /// Split by number of signatures
    BySignatures(usize),
}

// =============================================================================
// Statistics
// =============================================================================

/// Statistics about an imposition job
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpositionStatistics {
    /// Total number of source pages (including flyleaves)
    pub source_pages: usize,
    /// Total number of output sheets (physical pieces of paper)
    pub output_sheets: usize,
    /// Number of signatures (if using signature binding)
    pub signatures: Option<usize>,
    /// Pages per signature (if using signatures)
    pub pages_per_signature: Option<Vec<usize>>,
    /// Total output page count (usually output_sheets × 2)
    pub output_pages: usize,
    /// Number of blank pages added for padding
    pub blank_pages_added: usize,
}

impl ImpositionStatistics {
    /// Returns true if any blank pages were added
    pub fn has_blank_pages(&self) -> bool {
        self.blank_pages_added > 0
    }
}
