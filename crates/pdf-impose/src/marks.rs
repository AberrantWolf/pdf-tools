//! Printer's marks rendering for imposed pages
//!
//! This module provides functions to generate PDF content stream operations
//! for various printer's marks: fold lines, crop marks, registration marks, etc.
//!
//! Marks are rendered per-leaf (the folded/trimmed unit), not per-page.
//! This means crop marks appear at the corners of the entire leaf area,
//! while fold and cut lines appear at internal boundaries.

use crate::types::PrinterMarks;

/// Configuration for rendering marks on an imposed sheet.
///
/// The layout hierarchy is:
/// - Sheet: The entire output page (e.g., Letter, A3)
/// - Leaf area: The region inside sheet margins where content is placed
/// - Cells: Individual page positions within the leaf area (arranged in grid)
pub struct MarksConfig {
    /// Number of columns in the page grid
    pub cols: usize,
    /// Number of rows in the page grid
    pub rows: usize,
    /// Width of each cell (page position) in points
    pub cell_width: f32,
    /// Height of each cell (page position) in points
    pub cell_height: f32,
    /// Left edge of the leaf area in points (after sheet margins)
    pub leaf_left: f32,
    /// Bottom edge of the leaf area in points (after sheet margins)
    pub leaf_bottom: f32,
    /// Right edge of the leaf area in points
    pub leaf_right: f32,
    /// Top edge of the leaf area in points
    pub leaf_top: f32,
    /// Content boundaries for each cell (row-major order).
    /// Each entry is (x, y, width, height) of the actual content in that cell.
    /// Used for trim marks to show where content actually is, not just cell divisions.
    pub content_bounds: Vec<ContentBounds>,
}

/// Bounds of actual content within a cell
#[derive(Clone, Copy, Default)]
pub struct ContentBounds {
    /// X position of content (left edge)
    pub x: f32,
    /// Y position of content (bottom edge)
    pub y: f32,
    /// Width of content
    pub width: f32,
    /// Height of content
    pub height: f32,
}

/// Line weight for different mark types (in points)
const FOLD_LINE_WIDTH: f32 = 0.5;
const CUT_LINE_WIDTH: f32 = 0.5;
const CROP_MARK_WIDTH: f32 = 0.25;
const REGISTRATION_MARK_WIDTH: f32 = 0.25;

/// Length of crop marks in points
const CROP_MARK_LENGTH: f32 = 12.0;

/// Gap between crop mark and page edge
const CROP_MARK_GAP: f32 = 3.0;

/// Size of registration marks
const REGISTRATION_MARK_SIZE: f32 = 10.0;

/// Size of scissors symbol
const SCISSORS_SIZE: f32 = 8.0;

/// Generate all printer's marks as PDF content stream operations
pub fn generate_marks(marks: &PrinterMarks, config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Save graphics state
    ops.push_str("q\n");

    // Set default stroke color to black
    ops.push_str("0 0 0 RG\n");

    if marks.fold_lines {
        ops.push_str(&generate_fold_lines(config));
    }

    if marks.cut_lines {
        ops.push_str(&generate_cut_lines(config));
    }

    if marks.trim_marks {
        ops.push_str(&generate_trim_marks(config));
    }

    if marks.crop_marks {
        ops.push_str(&generate_crop_marks(config));
    }

    if marks.registration_marks {
        ops.push_str(&generate_registration_marks(config));
    }

    // Restore graphics state
    ops.push_str("Q\n");

    ops
}

/// Generate fold lines (dashed lines at fold positions)
/// For octavo (4 cols), the center vertical line is a cut, not a fold
fn generate_fold_lines(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties for fold lines
    ops.push_str(&format!("{} w\n", FOLD_LINE_WIDTH)); // line width
    ops.push_str("[6 3] 0 d\n"); // dashed line pattern: 6pt dash, 3pt gap

    // Vertical fold lines (between columns)
    // For 4-column layouts (octavo), the center line (col 2) is a cut, not a fold
    for col in 1..config.cols {
        // Skip center line for 4-column layouts - that's a cut line
        if config.cols == 4 && col == 2 {
            continue;
        }
        let x = config.leaf_left + col as f32 * config.cell_width;
        ops.push_str(&format!(
            "{} {} m {} {} l S\n",
            x, config.leaf_bottom, x, config.leaf_top
        ));
    }

    // Reset to solid line
    ops.push_str("[] 0 d\n");

    ops
}

/// Generate cut lines (solid lines with scissors at cut positions)
/// - Horizontal cuts between rows (quarto, octavo)
/// - Vertical center cut for octavo (4-column layouts)
fn generate_cut_lines(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties for cut lines
    ops.push_str(&format!("{} w\n", CUT_LINE_WIDTH)); // line width
    ops.push_str("[] 0 d\n"); // solid line

    // Horizontal cut lines (between rows)
    for row in 1..config.rows {
        let y = config.leaf_bottom + row as f32 * config.cell_height;
        ops.push_str(&format!(
            "{} {} m {} {} l S\n",
            config.leaf_left, y, config.leaf_right, y
        ));

        // Add scissors symbol at the left side of the cut line
        ops.push_str(&draw_scissors(config.leaf_left - SCISSORS_SIZE - 3.0, y));
    }

    // Vertical center cut for 4-column layouts (octavo)
    if config.cols == 4 {
        let x = config.leaf_left + 2.0 * config.cell_width; // Center line
        ops.push_str(&format!(
            "{} {} m {} {} l S\n",
            x, config.leaf_bottom, x, config.leaf_top
        ));

        // Add scissors symbol at the bottom of the vertical cut line
        ops.push_str(&draw_scissors_vertical(
            x,
            config.leaf_bottom - SCISSORS_SIZE - 3.0,
        ));
    }

    ops
}

/// Draw a scissors symbol at the given position
fn draw_scissors(x: f32, y: f32) -> String {
    let mut ops = String::new();
    let size = SCISSORS_SIZE;
    let half = size / 2.0;

    // Save state for scissors drawing
    ops.push_str("q\n");
    ops.push_str("0.3 w\n"); // thinner line for scissors

    // Draw two overlapping loops to represent scissors blades
    // Left blade (upper loop)
    let r = half * 0.4; // radius of the finger hole
    let k = r * 0.552284749831; // bezier control point factor

    // Upper loop center
    let cx1 = x + half * 0.3;
    let cy1 = y + half * 0.5;

    // Draw upper finger hole
    ops.push_str(&format!("{} {} m\n", cx1 + r, cy1));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 + r,
        cy1 + k,
        cx1 + k,
        cy1 + r,
        cx1,
        cy1 + r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 - k,
        cy1 + r,
        cx1 - r,
        cy1 + k,
        cx1 - r,
        cy1
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 - r,
        cy1 - k,
        cx1 - k,
        cy1 - r,
        cx1,
        cy1 - r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 + k,
        cy1 - r,
        cx1 + r,
        cy1 - k,
        cx1 + r,
        cy1
    ));
    ops.push_str("S\n");

    // Lower loop center
    let cx2 = x + half * 0.3;
    let cy2 = y - half * 0.5;

    // Draw lower finger hole
    ops.push_str(&format!("{} {} m\n", cx2 + r, cy2));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 + r,
        cy2 + k,
        cx2 + k,
        cy2 + r,
        cx2,
        cy2 + r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 - k,
        cy2 + r,
        cx2 - r,
        cy2 + k,
        cx2 - r,
        cy2
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 - r,
        cy2 - k,
        cx2 - k,
        cy2 - r,
        cx2,
        cy2 - r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 + k,
        cy2 - r,
        cx2 + r,
        cy2 - k,
        cx2 + r,
        cy2
    ));
    ops.push_str("S\n");

    // Draw the blades extending from the loops to the right
    // Upper blade
    ops.push_str(&format!(
        "{} {} m {} {} l S\n",
        cx1 + r,
        cy1 - r * 0.5,
        x + size,
        y + 1.0
    ));

    // Lower blade
    ops.push_str(&format!(
        "{} {} m {} {} l S\n",
        cx2 + r,
        cy2 + r * 0.5,
        x + size,
        y - 1.0
    ));

    // Restore state
    ops.push_str("Q\n");

    ops
}

/// Draw a scissors symbol rotated 90° for vertical cut lines
fn draw_scissors_vertical(x: f32, y: f32) -> String {
    let mut ops = String::new();
    let size = SCISSORS_SIZE;
    let half = size / 2.0;

    // Save state for scissors drawing
    ops.push_str("q\n");
    ops.push_str("0.3 w\n"); // thinner line for scissors

    // Draw two overlapping loops to represent scissors blades
    // Rotated 90° so blades point upward
    let r = half * 0.4; // radius of the finger hole
    let k = r * 0.552284749831; // bezier control point factor

    // Left loop center
    let cx1 = x - half * 0.5;
    let cy1 = y + half * 0.3;

    // Draw left finger hole
    ops.push_str(&format!("{} {} m\n", cx1 + r, cy1));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 + r,
        cy1 + k,
        cx1 + k,
        cy1 + r,
        cx1,
        cy1 + r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 - k,
        cy1 + r,
        cx1 - r,
        cy1 + k,
        cx1 - r,
        cy1
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 - r,
        cy1 - k,
        cx1 - k,
        cy1 - r,
        cx1,
        cy1 - r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx1 + k,
        cy1 - r,
        cx1 + r,
        cy1 - k,
        cx1 + r,
        cy1
    ));
    ops.push_str("S\n");

    // Right loop center
    let cx2 = x + half * 0.5;
    let cy2 = y + half * 0.3;

    // Draw right finger hole
    ops.push_str(&format!("{} {} m\n", cx2 + r, cy2));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 + r,
        cy2 + k,
        cx2 + k,
        cy2 + r,
        cx2,
        cy2 + r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 - k,
        cy2 + r,
        cx2 - r,
        cy2 + k,
        cx2 - r,
        cy2
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 - r,
        cy2 - k,
        cx2 - k,
        cy2 - r,
        cx2,
        cy2 - r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        cx2 + k,
        cy2 - r,
        cx2 + r,
        cy2 - k,
        cx2 + r,
        cy2
    ));
    ops.push_str("S\n");

    // Draw the blades extending from the loops upward
    // Left blade
    ops.push_str(&format!(
        "{} {} m {} {} l S\n",
        cx1 + r * 0.5,
        cy1 + r,
        x - 1.0,
        y + size
    ));

    // Right blade
    ops.push_str(&format!(
        "{} {} m {} {} l S\n",
        cx2 - r * 0.5,
        cy2 + r,
        x + 1.0,
        y + size
    ));

    // Restore state
    ops.push_str("Q\n");

    ops
}

/// Generate trim marks (L-shaped marks at corners of each cell/leaf position)
/// These marks show where each individual page should be trimmed after folding.
///
/// Trim marks are placed at the maximum content extent across all cells,
/// so varying aspect ratio content gets consistent trim boundaries.
fn generate_trim_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Find maximum content dimensions across all cells
    // This ensures trim marks encompass all content regardless of aspect ratio
    let mut max_content_width: f32 = 0.0;
    let mut max_content_height: f32 = 0.0;

    for bounds in &config.content_bounds {
        max_content_width = max_content_width.max(bounds.width);
        max_content_height = max_content_height.max(bounds.height);
    }

    // If no content bounds provided, fall back to cell dimensions
    if config.content_bounds.is_empty() || (max_content_width == 0.0 && max_content_height == 0.0) {
        max_content_width = config.cell_width;
        max_content_height = config.cell_height;
    }

    // Set line properties for trim marks
    ops.push_str(&format!("{} w\n", CROP_MARK_WIDTH));
    ops.push_str("[] 0 d\n"); // solid line

    // Draw trim marks at content boundaries for each cell
    // Content is aligned to fold edges, so we calculate trim position per cell
    for row in 0..config.rows {
        for col in 0..config.cols {
            // Cell origin (bottom-left)
            let cell_x = config.leaf_left + col as f32 * config.cell_width;
            let cell_y = config.leaf_bottom + (config.rows - row - 1) as f32 * config.cell_height;

            // Determine fold edges for this cell (content is pushed toward folds)
            let fold_on_right = match config.cols {
                2 => col == 0,
                4 => col == 0 || col == 2,
                _ => false,
            };
            let fold_on_left = match config.cols {
                2 => col == 1,
                4 => col == 1 || col == 3,
                _ => false,
            };
            let fold_on_bottom = config.rows > 1 && row == 0;
            let fold_on_top = config.rows > 1 && row == config.rows - 1;

            // Calculate content position based on fold alignment
            let content_x = if fold_on_right {
                cell_x + config.cell_width - max_content_width
            } else if fold_on_left {
                cell_x
            } else {
                cell_x + (config.cell_width - max_content_width) / 2.0
            };

            let content_y = if fold_on_bottom {
                cell_y
            } else if fold_on_top {
                cell_y + config.cell_height - max_content_height
            } else {
                cell_y + (config.cell_height - max_content_height) / 2.0
            };

            // Content corners
            let left = content_x;
            let right = content_x + max_content_width;
            let bottom = content_y;
            let top = content_y + max_content_height;

            // Draw L-shaped trim marks at each corner of the content area
            // Top-left corner
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                left,
                top + CROP_MARK_GAP,
                left,
                top + CROP_MARK_GAP + CROP_MARK_LENGTH
            ));
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                left - CROP_MARK_GAP,
                top,
                left - CROP_MARK_GAP - CROP_MARK_LENGTH,
                top
            ));

            // Top-right corner
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                right,
                top + CROP_MARK_GAP,
                right,
                top + CROP_MARK_GAP + CROP_MARK_LENGTH
            ));
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                right + CROP_MARK_GAP,
                top,
                right + CROP_MARK_GAP + CROP_MARK_LENGTH,
                top
            ));

            // Bottom-left corner
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                left,
                bottom - CROP_MARK_GAP,
                left,
                bottom - CROP_MARK_GAP - CROP_MARK_LENGTH
            ));
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                left - CROP_MARK_GAP,
                bottom,
                left - CROP_MARK_GAP - CROP_MARK_LENGTH,
                bottom
            ));

            // Bottom-right corner
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                right,
                bottom - CROP_MARK_GAP,
                right,
                bottom - CROP_MARK_GAP - CROP_MARK_LENGTH
            ));
            ops.push_str(&format!(
                "{} {} m {} {} l S\n",
                right + CROP_MARK_GAP,
                bottom,
                right + CROP_MARK_GAP + CROP_MARK_LENGTH,
                bottom
            ));
        }
    }

    ops
}

/// Generate crop marks (L-shaped marks at corners of the leaf area)
fn generate_crop_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties for crop marks
    ops.push_str(&format!("{} w\n", CROP_MARK_WIDTH));
    ops.push_str("[] 0 d\n"); // solid line

    // Draw crop marks at the four corners of the leaf area
    // Top-left corner
    ops.push_str(&crop_mark_top_left_top(config.leaf_left, config.leaf_top));
    ops.push_str(&crop_mark_top_left_left(config.leaf_left, config.leaf_top));

    // Top-right corner
    ops.push_str(&crop_mark_top_right_top(config.leaf_right, config.leaf_top));
    ops.push_str(&crop_mark_top_right_right(
        config.leaf_right,
        config.leaf_top,
    ));

    // Bottom-left corner
    ops.push_str(&crop_mark_bottom_left_bottom(
        config.leaf_left,
        config.leaf_bottom,
    ));
    ops.push_str(&crop_mark_bottom_left_left(
        config.leaf_left,
        config.leaf_bottom,
    ));

    // Bottom-right corner
    ops.push_str(&crop_mark_bottom_right_bottom(
        config.leaf_right,
        config.leaf_bottom,
    ));
    ops.push_str(&crop_mark_bottom_right_right(
        config.leaf_right,
        config.leaf_bottom,
    ));

    ops
}

// Individual crop mark drawing functions
fn crop_mark_top_left_top(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x,
        y + CROP_MARK_GAP,
        x,
        y + CROP_MARK_GAP + CROP_MARK_LENGTH
    )
}

fn crop_mark_top_left_left(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x - CROP_MARK_GAP,
        y,
        x - CROP_MARK_GAP - CROP_MARK_LENGTH,
        y
    )
}

fn crop_mark_top_right_top(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x,
        y + CROP_MARK_GAP,
        x,
        y + CROP_MARK_GAP + CROP_MARK_LENGTH
    )
}

fn crop_mark_top_right_right(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x + CROP_MARK_GAP,
        y,
        x + CROP_MARK_GAP + CROP_MARK_LENGTH,
        y
    )
}

fn crop_mark_bottom_left_bottom(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x,
        y - CROP_MARK_GAP,
        x,
        y - CROP_MARK_GAP - CROP_MARK_LENGTH
    )
}

fn crop_mark_bottom_left_left(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x - CROP_MARK_GAP,
        y,
        x - CROP_MARK_GAP - CROP_MARK_LENGTH,
        y
    )
}

fn crop_mark_bottom_right_bottom(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x,
        y - CROP_MARK_GAP,
        x,
        y - CROP_MARK_GAP - CROP_MARK_LENGTH
    )
}

fn crop_mark_bottom_right_right(x: f32, y: f32) -> String {
    format!(
        "{} {} m {} {} l S\n",
        x + CROP_MARK_GAP,
        y,
        x + CROP_MARK_GAP + CROP_MARK_LENGTH,
        y
    )
}

/// Generate registration marks (crosshair circles at midpoints of leaf edges)
/// Registration marks are placed at the center of each edge of the leaf area,
/// offset slightly outside for visibility without overlapping content.
fn generate_registration_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties
    ops.push_str(&format!("{} w\n", REGISTRATION_MARK_WIDTH));

    let offset = CROP_MARK_GAP + REGISTRATION_MARK_SIZE; // Position outside leaf edge
    let half_size = REGISTRATION_MARK_SIZE / 2.0;

    // Calculate midpoints of each edge
    let mid_x = (config.leaf_left + config.leaf_right) / 2.0;
    let mid_y = (config.leaf_top + config.leaf_bottom) / 2.0;

    // Draw registration marks at the midpoint of each edge (standard placement)
    let positions = [
        (mid_x, config.leaf_top + offset),    // Top center
        (mid_x, config.leaf_bottom - offset), // Bottom center
        (config.leaf_left - offset, mid_y),   // Left center
        (config.leaf_right + offset, mid_y),  // Right center
    ];

    for (x, y) in positions {
        ops.push_str(&draw_registration_mark(x, y, half_size));
    }

    ops
}

/// Draw a single registration mark (crosshair with circle)
fn draw_registration_mark(center_x: f32, center_y: f32, half_size: f32) -> String {
    let mut ops = String::new();

    // Draw crosshair
    // Horizontal line
    ops.push_str(&format!(
        "{} {} m {} {} l S\n",
        center_x - half_size,
        center_y,
        center_x + half_size,
        center_y
    ));

    // Vertical line
    ops.push_str(&format!(
        "{} {} m {} {} l S\n",
        center_x,
        center_y - half_size,
        center_x,
        center_y + half_size
    ));

    // Draw circle using Bezier curves (approximation)
    // For a circle, the control point distance is radius * 0.552284749831
    let r = half_size * 0.7; // Slightly smaller than crosshair
    let k = r * 0.552284749831;

    ops.push_str(&format!("{} {} m\n", center_x + r, center_y));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x + r,
        center_y + k,
        center_x + k,
        center_y + r,
        center_x,
        center_y + r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x - k,
        center_y + r,
        center_x - r,
        center_y + k,
        center_x - r,
        center_y
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x - r,
        center_y - k,
        center_x - k,
        center_y - r,
        center_x,
        center_y - r
    ));
    ops.push_str(&format!(
        "{} {} {} {} {} {} c\n",
        center_x + k,
        center_y - r,
        center_x + r,
        center_y - k,
        center_x + r,
        center_y
    ));
    ops.push_str("S\n");

    ops
}
