use thiserror::Error;

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

pub type Result<T> = std::result::Result<T, ImposeError>;

/// Paper orientation
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Orientation {
    /// Portrait: height > width (default for most paper sizes)
    #[default]
    Portrait,
    /// Landscape: width > height
    Landscape,
}

/// Standard paper sizes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaperSize {
    A3,
    A4,
    A5,
    Letter,
    Legal,
    Tabloid,
    Custom { width_mm: f32, height_mm: f32 },
}

impl PaperSize {
    /// Get base dimensions (always portrait: width < height for standard sizes)
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
}

/// Binding methods
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BindingType {
    /// Saddle-stitch binding (folded sheets, stapled at spine)
    Signature,
    /// Perfect binding (glued spine)
    PerfectBinding,
    /// Side stitch binding
    SideStitch,
    /// Spiral binding
    Spiral,
    /// Case binding (sewn signatures)
    CaseBinding,
}

/// Page arrangement methods
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageArrangement {
    /// Folio (single fold, 4 pages per sheet)
    Folio,
    /// Quarto (two folds, 8 pages per sheet)
    Quarto,
    /// Octavo (three folds, 16 pages per sheet)
    Octavo,
    /// Custom pages per signature
    Custom { pages_per_signature: usize },
}

impl PageArrangement {
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
}

/// Output PDF format
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    /// Single PDF with both sides interleaved
    DoubleSided,
    /// Two separate PDFs (fronts and backs)
    TwoSided,
    /// Single PDF with pages in print order for single-sided printing
    SingleSidedSequence,
}

/// Page scaling behavior when source pages don't match output size
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalingMode {
    /// Fit page to available space (preserve aspect ratio)
    Fit,
    /// Fill available space (may crop)
    Fill,
    /// Center without scaling
    None,
    /// Stretch to fill (ignore aspect ratio)
    Stretch,
}

/// Rotation for pages
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rotation {
    None,
    Clockwise90,
    Clockwise180,
    Clockwise270,
}

impl Rotation {
    pub fn degrees(self) -> i32 {
        match self {
            Rotation::None => 0,
            Rotation::Clockwise90 => 90,
            Rotation::Clockwise180 => 180,
            Rotation::Clockwise270 => 270,
        }
    }
}

/// Sheet margins - printer-safe area around the entire output sheet.
/// These margins ensure content stays within the printer's printable area.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SheetMargins {
    /// Top margin of the sheet
    pub top_mm: f32,
    /// Bottom margin of the sheet
    pub bottom_mm: f32,
    /// Left margin of the sheet
    pub left_mm: f32,
    /// Right margin of the sheet
    pub right_mm: f32,
}

impl Default for SheetMargins {
    fn default() -> Self {
        Self {
            top_mm: 5.0,
            bottom_mm: 5.0,
            left_mm: 5.0,
            right_mm: 5.0,
        }
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
}

/// Leaf margins - applied to each logical page within the imposed sheet.
/// These provide trim space for the binder and spine gutter for readability.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LeafMargins {
    /// Top margin (head) of each leaf
    pub top_mm: f32,
    /// Bottom margin (tail) of each leaf
    pub bottom_mm: f32,
    /// Outer margin (fore edge) - the edge that gets trimmed
    pub fore_edge_mm: f32,
    /// Inner margin (spine/gutter) - extra space near the binding
    pub spine_mm: f32,
}

impl Default for LeafMargins {
    fn default() -> Self {
        Self {
            top_mm: 5.0,
            bottom_mm: 5.0,
            fore_edge_mm: 5.0,
            spine_mm: 10.0,
        }
    }
}

/// Combined margins for imposition - both sheet-level and leaf-level
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Margins {
    /// Printer-safe margins around the entire output sheet
    pub sheet: SheetMargins,
    /// Margins for each logical page/leaf (trim and gutter)
    pub leaf: LeafMargins,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            sheet: SheetMargins::default(),
            leaf: LeafMargins::default(),
        }
    }
}

/// Printer's marks options
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PrinterMarks {
    /// Add fold lines (dashed) - where paper should be folded
    pub fold_lines: bool,
    /// Add cut lines (solid with scissors symbol) - where paper should be cut after folding
    pub cut_lines: bool,
    /// Add crop marks (L-shaped corner marks)
    pub crop_marks: bool,
    /// Add registration marks
    pub registration_marks: bool,
    /// Add sewing marks for sewn bindings
    pub sewing_marks: bool,
    /// Add spine marks for signature ordering
    pub spine_marks: bool,
}

/// Split output options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitMode {
    /// No splitting
    None,
    /// Split by number of pages
    ByPages(usize),
    /// Split by number of sheets
    BySheets(usize),
    /// Split by number of signatures
    BySignatures(usize),
}

/// Statistics about the imposition
#[derive(Debug, Clone, PartialEq)]
pub struct ImpositionStatistics {
    /// Total number of source pages
    pub source_pages: usize,
    /// Total number of output sheets
    pub output_sheets: usize,
    /// Number of signatures (if applicable)
    pub signatures: Option<usize>,
    /// Pages per signature (if using signatures)
    pub pages_per_signature: Option<Vec<usize>>,
    /// Output page count
    pub output_pages: usize,
    /// Number of blank pages added for padding
    pub blank_pages_added: usize,
}
