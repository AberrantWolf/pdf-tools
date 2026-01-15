//! Sheet rendering for imposition

use crate::constants::{
    DEFAULT_PAGE_DIMENSIONS, HELVETICA_CHAR_WIDTH_RATIO, PAGE_NUMBER_FONT_SIZE, PAGE_NUMBER_OFFSET,
};
use crate::layout::{
    GridLayout, PagePlacement, SheetLayout, SignatureSlot, calculate_content_area, cell_bounds,
    place_page,
};
use crate::marks::{ContentBounds, MarksConfig, generate_marks};
use crate::options::ImpositionOptions;
use crate::render::create_page_xobject;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashMap;

// =============================================================================
// Placement Calculation
// =============================================================================

/// Calculate page placements for one side of a sheet
pub(crate) fn calculate_sheet_placements(
    grid: &GridLayout,
    slots: &[&SignatureSlot],
    page_mapping: &[Option<usize>],
    source_dimensions: &[(f32, f32)],
    leaf_margins: &LeafMargins,
    scaling_mode: ScalingMode,
    leaf_origin: (f32, f32),
) -> Vec<PagePlacement> {
    slots
        .iter()
        .zip(page_mapping.iter())
        .map(|(slot, &source_page)| {
            let cell = cell_bounds(grid, slot.grid_pos, leaf_origin);
            let content_area = calculate_content_area(&cell, leaf_margins, slot, grid);

            let (src_width, src_height) = source_page
                .and_then(|idx| source_dimensions.get(idx).copied())
                .unwrap_or(DEFAULT_PAGE_DIMENSIONS);

            let mut placement = place_page(
                &content_area,
                src_width,
                src_height,
                scaling_mode,
                slot,
                grid,
            );
            placement.source_page = source_page;
            placement
        })
        .collect()
}

// =============================================================================
// Sheet Rendering
// =============================================================================

/// Render one side of a sheet to the output document
pub(crate) fn render_sheet(
    output: &mut Document,
    source: &Document,
    source_page_ids: &[ObjectId],
    layout: &SheetLayout,
    sheet_width_pt: f32,
    sheet_height_pt: f32,
    parent_pages_id: ObjectId,
    grid: &GridLayout,
    options: &ImpositionOptions,
) -> Result<ObjectId> {
    let mut page_dict = create_page_dict(parent_pages_id, sheet_width_pt, sheet_height_pt);

    let mut content_ops = Vec::new();
    let mut xobjects = Dictionary::new();
    let mut fonts = Dictionary::new();
    let mut xobject_cache: HashMap<ObjectId, ObjectId> = HashMap::new();
    let mut content_bounds: Vec<ContentBounds> = Vec::new();

    // Render each page placement
    for (idx, placement) in layout.placements.iter().enumerate() {
        if let Some(source_idx) = placement.source_page {
            if source_idx < source_page_ids.len() {
                let source_page_id = source_page_ids[source_idx];
                let xobject_name = format!("P{}", idx);

                // Create XObject
                let xobject_id =
                    create_page_xobject(output, source, source_page_id, &mut xobject_cache)?;
                xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                // Generate placement command
                content_ops.push(generate_placement_cmd(&xobject_name, placement));

                // Record bounds for marks
                content_bounds.push(ContentBounds {
                    x: placement.content_rect.x,
                    y: placement.content_rect.y,
                    width: placement.content_rect.width,
                    height: placement.content_rect.height,
                });
            }
        }
    }

    // Generate printer's marks
    if options.marks.any_enabled() {
        let marks_config = MarksConfig {
            cols: grid.cols,
            rows: grid.rows,
            cell_width: grid.cell_width_pt,
            cell_height: grid.cell_height_pt,
            leaf_left: layout.leaf_bounds.x,
            leaf_bottom: layout.leaf_bounds.y,
            leaf_right: layout.leaf_bounds.right(),
            leaf_top: layout.leaf_bounds.top(),
            content_bounds,
        };
        content_ops.push(generate_marks(&options.marks, &marks_config));
    }

    // Add page numbers
    if options.add_page_numbers {
        let (font_ops, font_id) = render_page_numbers(output, layout, grid, options);
        content_ops.push(font_ops);
        fonts.set("F1", Object::Reference(font_id));
    }

    // Build resources
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

    Ok(output.add_object(page_dict))
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a basic page dictionary
fn create_page_dict(parent_id: ObjectId, width: f32, height: f32) -> Dictionary {
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"Page".to_vec()));
    dict.set("Parent", Object::Reference(parent_id));
    dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(width),
            Object::Real(height),
        ]),
    );
    dict
}

/// Generate PDF command to place an XObject
fn generate_placement_cmd(xobject_name: &str, placement: &PagePlacement) -> String {
    let rect = &placement.content_rect;

    if placement.is_rotated() {
        // 180° rotation: matrix is [-scale 0 0 -scale tx ty]
        let rot_x = rect.x + rect.width;
        let rot_y = rect.y + rect.height;
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            -placement.scale, -placement.scale, rot_x, rot_y, xobject_name
        )
    } else {
        format!(
            "q {} 0 0 {} {} {} cm /{} Do Q\n",
            placement.scale, placement.scale, rect.x, rect.y, xobject_name
        )
    }
}

/// Render page numbers and return (content ops, font object id)
fn render_page_numbers(
    output: &mut Document,
    layout: &SheetLayout,
    grid: &GridLayout,
    options: &ImpositionOptions,
) -> (String, ObjectId) {
    // Create font
    let mut font_dict = Dictionary::new();
    font_dict.set("Type", Object::Name(b"Font".to_vec()));
    font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
    font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
    let font_id = output.add_object(font_dict);

    let mut ops = String::new();

    for placement in &layout.placements {
        if let Some(source_idx) = placement.source_page {
            let page_num = options.page_number_start + source_idx;
            let page_num_text = page_num.to_string();

            // Calculate cell position
            let cell_x =
                layout.leaf_bounds.x + placement.slot.grid_pos.col as f32 * grid.cell_width_pt;
            let cell_y = layout.leaf_bounds.y
                + (grid.rows - placement.slot.grid_pos.row - 1) as f32 * grid.cell_height_pt;

            if placement.is_rotated() {
                // Rotated: position at top (appears at bottom after rotation)
                let text_x = cell_x + grid.cell_width_pt / 2.0;
                let text_y = cell_y + grid.cell_height_pt - PAGE_NUMBER_OFFSET;
                ops.push_str(&format!(
                    "q 1 0 0 1 {} {} cm -1 0 0 -1 0 0 cm BT /F1 {} Tf ({}) Tj ET Q\n",
                    text_x, text_y, PAGE_NUMBER_FONT_SIZE, page_num_text
                ));
            } else {
                // Normal: position at bottom center
                let text_width =
                    page_num_text.len() as f32 * PAGE_NUMBER_FONT_SIZE * HELVETICA_CHAR_WIDTH_RATIO;
                let text_x = cell_x + grid.cell_width_pt / 2.0 - text_width / 2.0;
                let text_y = cell_y + PAGE_NUMBER_OFFSET;
                ops.push_str(&format!(
                    "BT /F1 {} Tf {} {} Td ({}) Tj ET\n",
                    PAGE_NUMBER_FONT_SIZE, text_x, text_y, page_num_text
                ));
            }
        }
    }

    (ops, font_id)
}
