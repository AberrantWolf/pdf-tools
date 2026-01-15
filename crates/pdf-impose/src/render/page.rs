//! Output page rendering for imposition
//!
//! This module creates the final imposed PDF pages by placing
//! source pages (as XObjects) with appropriate transformations.

use crate::layout::{PagePlacement, Rect};
use crate::marks::{ContentBounds, MarksConfig, generate_marks};
use crate::types::{PrinterMarks, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashMap;

use super::xobject::create_page_xobject;

/// Render an imposed output page.
///
/// # Arguments
/// * `output` - The output document
/// * `source` - The source document containing the pages
/// * `source_page_ids` - Object IDs of all source pages
/// * `placements` - Page placements for this output page
/// * `sheet_width_pt` - Output page width in points
/// * `sheet_height_pt` - Output page height in points
/// * `parent_pages_id` - The parent Pages object ID
/// * `marks` - Printer's marks configuration
/// * `leaf_bounds` - The leaf area bounds (for marks)
/// * `grid_cols` - Number of columns in the grid
/// * `grid_rows` - Number of rows in the grid
/// * `cell_width` - Width of each cell in points
/// * `cell_height` - Height of each cell in points
/// * `add_page_numbers` - Whether to add page numbers
/// * `page_number_start` - Starting page number
pub fn render_imposed_page(
    output: &mut Document,
    source: &Document,
    source_page_ids: &[ObjectId],
    placements: &[PagePlacement],
    sheet_width_pt: f32,
    sheet_height_pt: f32,
    parent_pages_id: ObjectId,
    marks: &PrinterMarks,
    leaf_bounds: &Rect,
    grid_cols: usize,
    grid_rows: usize,
    cell_width: f32,
    cell_height: f32,
    add_page_numbers: bool,
    page_number_start: usize,
) -> Result<ObjectId> {
    // Create page dictionary
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(sheet_width_pt),
            Object::Real(sheet_height_pt),
        ]),
    );

    let mut content_ops = Vec::new();
    let mut xobjects = Dictionary::new();
    let mut fonts = Dictionary::new();
    let mut xobject_cache: HashMap<ObjectId, ObjectId> = HashMap::new();

    // Collect content bounds for marks
    let mut content_bounds: Vec<ContentBounds> = Vec::new();

    // Render each page placement
    for (idx, placement) in placements.iter().enumerate() {
        if let Some(source_idx) = placement.source_page {
            if source_idx < source_page_ids.len() {
                let source_page_id = source_page_ids[source_idx];

                // Create XObject for this source page
                let xobject_name = format!("P{}", idx);
                let xobject_id =
                    create_page_xobject(output, source, source_page_id, &mut xobject_cache)?;
                xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                // Generate transformation and draw command
                let cmd = generate_placement_command(
                    &xobject_name,
                    &placement.content_rect,
                    placement.scale,
                    placement.rotation_degrees,
                );
                content_ops.push(cmd);

                // Record content bounds for marks
                content_bounds.push(ContentBounds {
                    x: placement.content_rect.x,
                    y: placement.content_rect.y,
                    width: placement.content_rect.width,
                    height: placement.content_rect.height,
                });
            }
        }
    }

    // Generate printer's marks if enabled
    let has_marks = marks.fold_lines
        || marks.cut_lines
        || marks.crop_marks
        || marks.registration_marks
        || marks.trim_marks;

    if has_marks {
        let marks_config = MarksConfig {
            cols: grid_cols,
            rows: grid_rows,
            cell_width,
            cell_height,
            leaf_left: leaf_bounds.x,
            leaf_bottom: leaf_bounds.y,
            leaf_right: leaf_bounds.right(),
            leaf_top: leaf_bounds.top(),
            content_bounds,
        };
        let marks_content = generate_marks(marks, &marks_config);
        content_ops.push(marks_content);
    }

    // Add page numbers if enabled
    if add_page_numbers {
        let (font_ops, font_dict) = render_page_numbers(
            placements,
            page_number_start,
            cell_width,
            cell_height,
            leaf_bounds,
            grid_rows,
        );
        content_ops.push(font_ops);
        fonts = font_dict;
    }

    // Set up resources
    let mut resources = Dictionary::new();
    resources.set("XObject", Object::Dictionary(xobjects));
    if !fonts.is_empty() {
        resources.set("Font", Object::Dictionary(fonts));
    }

    // Create content stream
    let content = content_ops.join("");
    let content_id = output.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(resources));

    // Add page to document
    let page_id = output.add_object(page_dict);
    Ok(page_id)
}

/// Generate the PDF content stream command to place a page.
fn generate_placement_command(
    xobject_name: &str,
    rect: &Rect,
    scale: f32,
    rotation_degrees: f32,
) -> String {
    if rotation_degrees.abs() > 0.1 {
        // 180° rotation: matrix is [-scale 0 0 -scale tx ty]
        // where tx, ty is the rotation point (top-right of content)
        let rot_x = rect.x + rect.width;
        let rot_y = rect.y + rect.height;
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            -scale, -scale, rot_x, rot_y, xobject_name
        )
    } else {
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            scale, scale, rect.x, rect.y, xobject_name
        )
    }
}

/// Render page numbers onto the output page.
fn render_page_numbers(
    placements: &[PagePlacement],
    page_number_start: usize,
    cell_width: f32,
    cell_height: f32,
    leaf_bounds: &Rect,
    grid_rows: usize,
) -> (String, Dictionary) {
    let mut ops = String::new();
    let mut fonts = Dictionary::new();

    // We need to create the font here - the caller will add it to output doc
    let mut font_dict = Dictionary::new();
    font_dict.set("Type", Object::Name(b"Font".to_vec()));
    font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
    font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    // Note: This creates an orphan dictionary - in real use, add to output doc
    fonts.set("F1", Object::Dictionary(font_dict));

    let font_size = 8.0;

    for placement in placements {
        if let Some(source_idx) = placement.source_page {
            let page_num = page_number_start + source_idx;
            let grid_pos = &placement.slot.grid_pos;

            // Calculate cell position
            let cell_x = leaf_bounds.x + grid_pos.col as f32 * cell_width;
            let cell_y = leaf_bounds.y + (grid_rows - grid_pos.row - 1) as f32 * cell_height;

            let page_num_text = page_num.to_string();

            if placement.rotation_degrees.abs() > 0.1 {
                // Rotated page: position at top (appears at bottom after rotation)
                let text_x = cell_x + cell_width / 2.0;
                let text_y = cell_y + cell_height - 10.0;
                ops.push_str(&format!(
                    "q 1 0 0 1 {} {} cm -1 0 0 -1 0 0 cm BT /F1 {} Tf ({}) Tj ET Q\n",
                    text_x, text_y, font_size, page_num_text
                ));
            } else {
                // Normal page: position at bottom center
                let text_width = page_num_text.len() as f32 * font_size * 0.5;
                let text_x = cell_x + cell_width / 2.0 - text_width / 2.0;
                let text_y = cell_y + 10.0;
                ops.push_str(&format!(
                    "BT /F1 {} Tf {} {} Td ({}) Tj ET\n",
                    font_size, text_x, text_y, page_num_text
                ));
            }
        }
    }

    (ops, fonts)
}
