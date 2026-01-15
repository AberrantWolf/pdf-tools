//! Simple 2-up binding imposition (perfect binding, side stitch, spiral)

use super::sheet::{calculate_sheet_placements, render_sheet};
use super::sheet_dimensions_pt;
use crate::constants::mm_to_pt;
use crate::layout::{PageSide, Rect, SheetLayout, SheetSide, SignatureSlot, create_grid_layout};
use crate::options::ImpositionOptions;
use crate::render::get_page_dimensions;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Impose using simple 2-up binding (perfect binding, side stitch, spiral)
///
/// Each output page has 2 source pages side by side.
pub(crate) fn impose_simple_binding(
    source: &Document,
    page_ids: &[ObjectId],
    options: &ImpositionOptions,
) -> Result<Document> {
    let total_pages = page_ids.len();

    // Get source page dimensions
    let source_dimensions: Vec<(f32, f32)> = page_ids
        .iter()
        .map(|&id| {
            get_page_dimensions(source, id).unwrap_or(crate::constants::DEFAULT_PAGE_DIMENSIONS)
        })
        .collect();

    // Calculate output dimensions and leaf area
    let (output_width_pt, output_height_pt) = sheet_dimensions_pt(options);
    let leaf_bounds = calculate_leaf_bounds(options, output_width_pt, output_height_pt);

    // Simple 2-up grid (use folio layout)
    let grid = create_grid_layout(
        PageArrangement::Folio,
        leaf_bounds.width,
        leaf_bounds.height,
        output_width_pt,
        output_height_pt,
    );

    // Build output document
    let mut output = Document::with_version("1.7");
    let pages_tree_id = output.new_object_id();
    let mut page_refs = Vec::new();

    // Pad to even number
    let padded_count = (total_pages + 1) / 2 * 2;

    // Process pages in pairs
    for chunk_start in (0..padded_count).step_by(2) {
        let left_page = if chunk_start < total_pages {
            Some(chunk_start)
        } else {
            None
        };
        let right_page = if chunk_start + 1 < total_pages {
            Some(chunk_start + 1)
        } else {
            None
        };

        // Create simple slots for 2-up layout
        let left_slot = SignatureSlot::new(0, SheetSide::Front, 0, 0, false, PageSide::Verso);
        let right_slot = SignatureSlot::new(1, SheetSide::Front, 0, 1, false, PageSide::Recto);

        let slots = vec![&left_slot, &right_slot];
        let page_mapping = vec![left_page, right_page];

        let placements = calculate_sheet_placements(
            &grid,
            &slots,
            &page_mapping,
            &source_dimensions,
            &options.margins.leaf,
            options.scaling_mode,
            (leaf_bounds.x, leaf_bounds.y),
        );

        let layout = SheetLayout {
            side: SheetSide::Front,
            placements,
            leaf_bounds,
        };

        let page_id = render_sheet(
            &mut output,
            source,
            page_ids,
            &layout,
            output_width_pt,
            output_height_pt,
            pages_tree_id,
            &grid,
            options,
        )?;
        page_refs.push(Object::Reference(page_id));
    }

    // Finalize document
    finalize_document(&mut output, pages_tree_id, page_refs);
    Ok(output)
}

/// Calculate the leaf area bounds (inside sheet margins)
fn calculate_leaf_bounds(options: &ImpositionOptions, width_pt: f32, height_pt: f32) -> Rect {
    let margins = &options.margins.sheet;
    Rect::new(
        mm_to_pt(margins.left_mm),
        mm_to_pt(margins.bottom_mm),
        width_pt - mm_to_pt(margins.left_mm) - mm_to_pt(margins.right_mm),
        height_pt - mm_to_pt(margins.top_mm) - mm_to_pt(margins.bottom_mm),
    )
}

/// Create pages tree and catalog, finalize document structure
fn finalize_document(output: &mut Document, pages_tree_id: ObjectId, page_refs: Vec<Object>) {
    let count = page_refs.len() as i64;
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(page_refs)),
        ("Count", Object::Integer(count)),
    ]);
    output
        .objects
        .insert(pages_tree_id, Object::Dictionary(pages_dict));

    let catalog_id = output.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_tree_id)),
    ]));

    output.trailer.set("Root", catalog_id);
}
