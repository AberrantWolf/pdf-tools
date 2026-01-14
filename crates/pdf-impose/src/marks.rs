//! Printer's marks rendering for imposed pages
//!
//! This module provides functions to generate PDF content stream operations
//! for various printer's marks: fold lines, crop marks, registration marks, etc.

use crate::types::PrinterMarks;

/// Configuration for rendering marks
pub struct MarksConfig {
    /// Number of columns in the grid
    pub cols: usize,
    /// Number of rows in the grid
    pub rows: usize,
    /// Cell width in points
    pub cell_width: f32,
    /// Cell height in points
    pub cell_height: f32,
    /// Left margin (fore-edge) in points - where content area starts
    pub margin_left: f32,
    /// Bottom margin in points - where content area starts
    pub margin_bottom: f32,
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

    // Content area bounds
    let content_left = config.margin_left;
    let content_bottom = config.margin_bottom;
    let content_top = content_bottom + (config.rows as f32 * config.cell_height);

    // Vertical fold lines (between columns)
    // For 4-column layouts (octavo), the center line (col 2) is a cut, not a fold
    for col in 1..config.cols {
        // Skip center line for 4-column layouts - that's a cut line
        if config.cols == 4 && col == 2 {
            continue;
        }
        let x = content_left + col as f32 * config.cell_width;
        ops.push_str(&format!(
            "{} {} m {} {} l S\n",
            x, content_bottom, x, content_top
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

    // Content area bounds
    let content_left = config.margin_left;
    let content_bottom = config.margin_bottom;
    let content_right = content_left + (config.cols as f32 * config.cell_width);
    let content_top = content_bottom + (config.rows as f32 * config.cell_height);

    // Horizontal cut lines (between rows)
    for row in 1..config.rows {
        let y = content_bottom + row as f32 * config.cell_height;
        ops.push_str(&format!(
            "{} {} m {} {} l S\n",
            content_left, y, content_right, y
        ));

        // Add scissors symbol at the left side of the cut line
        ops.push_str(&draw_scissors(content_left - SCISSORS_SIZE - 3.0, y));
    }

    // Vertical center cut for 4-column layouts (octavo)
    if config.cols == 4 {
        let x = content_left + 2.0 * config.cell_width; // Center line
        ops.push_str(&format!(
            "{} {} m {} {} l S\n",
            x, content_bottom, x, content_top
        ));

        // Add scissors symbol at the bottom of the vertical cut line
        ops.push_str(&draw_scissors_vertical(
            x,
            content_bottom - SCISSORS_SIZE - 3.0,
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

/// Generate crop marks (L-shaped marks at corners of the content area)
fn generate_crop_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties for crop marks
    ops.push_str(&format!("{} w\n", CROP_MARK_WIDTH));
    ops.push_str("[] 0 d\n"); // solid line

    // Content area bounds
    let content_left = config.margin_left;
    let content_bottom = config.margin_bottom;
    let content_right = content_left + (config.cols as f32 * config.cell_width);
    let content_top = content_bottom + (config.rows as f32 * config.cell_height);

    // Draw crop marks at the four corners of the content area
    // Top-left corner
    ops.push_str(&crop_mark_top_left_top(content_left, content_top));
    ops.push_str(&crop_mark_top_left_left(content_left, content_top));

    // Top-right corner
    ops.push_str(&crop_mark_top_right_top(content_right, content_top));
    ops.push_str(&crop_mark_top_right_right(content_right, content_top));

    // Bottom-left corner
    ops.push_str(&crop_mark_bottom_left_bottom(content_left, content_bottom));
    ops.push_str(&crop_mark_bottom_left_left(content_left, content_bottom));

    // Bottom-right corner
    ops.push_str(&crop_mark_bottom_right_bottom(
        content_right,
        content_bottom,
    ));
    ops.push_str(&crop_mark_bottom_right_right(content_right, content_bottom));

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

/// Generate registration marks (crosshair circles at corners of content area)
fn generate_registration_marks(config: &MarksConfig) -> String {
    let mut ops = String::new();

    // Set line properties
    ops.push_str(&format!("{} w\n", REGISTRATION_MARK_WIDTH));

    // Content area bounds
    let content_left = config.margin_left;
    let content_bottom = config.margin_bottom;
    let content_right = content_left + (config.cols as f32 * config.cell_width);
    let content_top = content_bottom + (config.rows as f32 * config.cell_height);

    let offset = CROP_MARK_GAP + CROP_MARK_LENGTH + 5.0; // Position beyond crop marks
    let half_size = REGISTRATION_MARK_SIZE / 2.0;

    // Draw registration marks at the four corners of the content area
    let positions = [
        (content_left - offset, content_top + offset), // Top-left
        (content_right + offset, content_top + offset), // Top-right
        (content_left - offset, content_bottom - offset), // Bottom-left
        (content_right + offset, content_bottom - offset), // Bottom-right
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
