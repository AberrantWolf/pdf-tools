//! Shared constants for PDF imposition
//!
//! This module centralizes magic numbers and constants used throughout
//! the imposition process.

// =============================================================================
// Unit Conversion
// =============================================================================

/// Points per millimeter (1 inch = 72 points, 1 inch = 25.4mm)
pub const POINTS_PER_MM: f32 = 72.0 / 25.4; // ≈ 2.83465

/// Convert millimeters to points
#[inline]
pub fn mm_to_pt(mm: f32) -> f32 {
    mm * POINTS_PER_MM
}

/// Convert points to millimeters
#[inline]
pub fn pt_to_mm(pt: f32) -> f32 {
    pt / POINTS_PER_MM
}

// =============================================================================
// Default Page Dimensions
// =============================================================================

/// Default page width in points (US Letter: 8.5" × 11")
pub const DEFAULT_PAGE_WIDTH_PT: f32 = 612.0;

/// Default page height in points (US Letter)
pub const DEFAULT_PAGE_HEIGHT_PT: f32 = 792.0;

/// Default page dimensions as tuple (width, height)
pub const DEFAULT_PAGE_DIMENSIONS: (f32, f32) = (DEFAULT_PAGE_WIDTH_PT, DEFAULT_PAGE_HEIGHT_PT);

// =============================================================================
// Printer's Marks
// =============================================================================

/// Line width for fold lines (points)
pub const FOLD_LINE_WIDTH: f32 = 0.5;

/// Line width for cut lines (points)
pub const CUT_LINE_WIDTH: f32 = 0.5;

/// Line width for crop marks (points)
pub const CROP_MARK_WIDTH: f32 = 0.25;

/// Line width for registration marks (points)
pub const REGISTRATION_MARK_WIDTH: f32 = 0.25;

/// Length of crop marks (points)
pub const CROP_MARK_LENGTH: f32 = 12.0;

/// Gap between crop mark and content edge (points)
pub const CROP_MARK_GAP: f32 = 3.0;

/// Size of registration marks (points)
pub const REGISTRATION_MARK_SIZE: f32 = 10.0;

/// Size of scissors symbol (points)
pub const SCISSORS_SIZE: f32 = 8.0;

// =============================================================================
// Page Numbers
// =============================================================================

/// Default font size for page numbers (points)
pub const PAGE_NUMBER_FONT_SIZE: f32 = 8.0;

/// Vertical offset for page numbers from cell edge (points)
pub const PAGE_NUMBER_OFFSET: f32 = 10.0;

/// Approximate character width ratio for Helvetica
pub const HELVETICA_CHAR_WIDTH_RATIO: f32 = 0.5;

// =============================================================================
// Bezier Curve Constants
// =============================================================================

/// Control point factor for approximating circles with Bezier curves.
/// This magic number comes from: 4 * (sqrt(2) - 1) / 3 ≈ 0.552284749831
/// Using 4 cubic Bezier curves with this factor gives a very close circle approximation.
pub const BEZIER_CIRCLE_FACTOR: f32 = 0.552284749831;

// =============================================================================
// Flyleaves
// =============================================================================

/// Pages per leaf (front and back sides)
pub const PAGES_PER_LEAF: usize = 2;
