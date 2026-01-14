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

/// Paper sizes in millimeters (width, height)
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

/// Margins for page layout
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Margins {
    /// Top margin (head)
    pub top_mm: f32,
    /// Bottom margin (tail)
    pub bottom_mm: f32,
    /// Outer margin (fore edge)
    pub fore_edge_mm: f32,
    /// Inner margin (spine)
    pub spine_mm: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top_mm: 10.0,
            bottom_mm: 10.0,
            fore_edge_mm: 10.0,
            spine_mm: 15.0,
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
