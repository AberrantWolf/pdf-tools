//! Sheet rendering for imposition

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

            // Get source dimensions
            let (src_width, src_height) = source_page
                .and_then(|idx| source_dimensions.get(idx).copied())
                .unwrap_or((612.0, 792.0));

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
    for (idx, placement) in layout.placements.iter().enumerate() {
        if let Some(source_idx) = placement.source_page {
            if source_idx < source_page_ids.len() {
                let source_page_id = source_page_ids[source_idx];

                // Create XObject
                let xobject_name = format!("P{}", idx);
                let xobject_id =
                    create_page_xobject(output, source, source_page_id, &mut xobject_cache)?;
                xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                // Generate transform command
                let rect = &placement.content_rect;
                let cmd = if placement.rotation_degrees.abs() > 0.1 {
                    // 180° rotation
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
                };
                content_ops.push(cmd);

                // Record bounds for marks
                content_bounds.push(ContentBounds {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                });
            }
        }
    }

    // Generate printer's marks
    let has_marks = options.marks.fold_lines
        || options.marks.cut_lines
        || options.marks.crop_marks
        || options.marks.registration_marks
        || options.marks.trim_marks;

    if has_marks {
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
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        let font_id = output.add_object(font_dict);
        fonts.set("F1", Object::Reference(font_id));

        let font_size = 8.0;
        for placement in &layout.placements {
            if let Some(source_idx) = placement.source_page {
                let page_num = options.page_number_start + source_idx;
                let grid_pos = &placement.slot.grid_pos;

                let cell_x = layout.leaf_bounds.x + grid_pos.col as f32 * grid.cell_width_pt;
                let cell_y = layout.leaf_bounds.y
                    + (grid.rows - grid_pos.row - 1) as f32 * grid.cell_height_pt;

                let page_num_text = page_num.to_string();

                if placement.rotation_degrees.abs() > 0.1 {
                    let text_x = cell_x + grid.cell_width_pt / 2.0;
                    let text_y = cell_y + grid.cell_height_pt - 10.0;
                    content_ops.push(format!(
                        "q 1 0 0 1 {} {} cm -1 0 0 -1 0 0 cm BT /F1 {} Tf ({}) Tj ET Q\n",
                        text_x, text_y, font_size, page_num_text
                    ));
                } else {
                    let text_width = page_num_text.len() as f32 * font_size * 0.5;
                    let text_x = cell_x + grid.cell_width_pt / 2.0 - text_width / 2.0;
                    let text_y = cell_y + 10.0;
                    content_ops.push(format!(
                        "BT /F1 {} Tf {} {} Td ({}) Tj ET\n",
                        font_size, text_x, text_y, page_num_text
                    ));
                }
            }
        }
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

    let page_id = output.add_object(page_dict);
    Ok(page_id)
}
