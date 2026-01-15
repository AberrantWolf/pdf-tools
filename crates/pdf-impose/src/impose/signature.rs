//! Signature binding imposition (folded sheets)

use super::sheet::{calculate_sheet_placements, render_sheet};
use super::sheet_dimensions_pt;
use crate::constants::mm_to_pt;
use crate::layout::{
    Rect, SheetLayout, SheetSide, calculate_signature_slots, create_grid_layout, map_pages_to_slots,
};
use crate::options::ImpositionOptions;
use crate::render::get_page_dimensions;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId};

/// Impose using signature binding (folded sheets)
pub(crate) fn impose_signature_binding(
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

    // Create grid layout
    let grid = create_grid_layout(
        options.page_arrangement,
        leaf_bounds.width,
        leaf_bounds.height,
        output_width_pt,
        output_height_pt,
    );

    // Calculate signature slots
    let signatures = calculate_signature_slots(total_pages, options.page_arrangement);

    // Build output document
    let mut output = Document::with_version("1.7");
    let pages_tree_id = output.new_object_id();
    let mut page_refs = Vec::new();

    // Process each signature
    for (sig_num, sig_slots) in signatures.iter().enumerate() {
        let sig_start = sig_num * options.page_arrangement.pages_per_signature();

        // Map source pages to slots
        let page_mapping = map_pages_to_slots(options.page_arrangement, sig_start, total_pages);

        // Split slots by sheet side
        let front_slots: Vec<_> = sig_slots
            .iter()
            .filter(|s| s.sheet_side == SheetSide::Front)
            .collect();
        let back_slots: Vec<_> = sig_slots
            .iter()
            .filter(|s| s.sheet_side == SheetSide::Back)
            .collect();

        // Render front side
        let front_placements = calculate_sheet_placements(
            &grid,
            &front_slots,
            &page_mapping[..front_slots.len()],
            &source_dimensions,
            &options.margins.leaf,
            options.scaling_mode,
            (leaf_bounds.x, leaf_bounds.y),
        );

        let front_layout = SheetLayout {
            side: SheetSide::Front,
            placements: front_placements,
            leaf_bounds,
        };

        let front_page_id = render_sheet(
            &mut output,
            source,
            page_ids,
            &front_layout,
            output_width_pt,
            output_height_pt,
            pages_tree_id,
            &grid,
            options,
        )?;
        page_refs.push(Object::Reference(front_page_id));

        // Render back side
        if !back_slots.is_empty() {
            let back_placements = calculate_sheet_placements(
                &grid,
                &back_slots,
                &page_mapping[front_slots.len()..],
                &source_dimensions,
                &options.margins.leaf,
                options.scaling_mode,
                (leaf_bounds.x, leaf_bounds.y),
            );

            let back_layout = SheetLayout {
                side: SheetSide::Back,
                placements: back_placements,
                leaf_bounds,
            };

            let back_page_id = render_sheet(
                &mut output,
                source,
                page_ids,
                &back_layout,
                output_width_pt,
                output_height_pt,
                pages_tree_id,
                &grid,
                options,
            )?;
            page_refs.push(Object::Reference(back_page_id));
        }
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
