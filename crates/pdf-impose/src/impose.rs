use crate::marks::{MarksConfig, generate_marks};
use crate::options::ImpositionOptions;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, Stream};
use std::collections::HashMap;
use std::path::Path;

/// Load a single PDF document
pub async fn load_pdf(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref().to_owned();
    let bytes = tokio::fs::read(&path).await?;
    let doc = tokio::task::spawn_blocking(move || Document::load_mem(&bytes)).await??;
    Ok(doc)
}

/// Load multiple PDF documents
pub async fn load_multiple_pdfs(paths: &[impl AsRef<Path>]) -> Result<Vec<Document>> {
    let mut documents = Vec::new();
    for path in paths {
        documents.push(load_pdf(path).await?);
    }
    Ok(documents)
}

/// Save the imposed document
pub async fn save_pdf(mut doc: Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref().to_owned();
    let bytes = tokio::task::spawn_blocking(move || {
        let mut writer = Vec::new();
        doc.save_to(&mut writer)?;
        Ok::<_, ImposeError>(writer)
    })
    .await??;
    tokio::fs::write(&path, bytes).await?;
    Ok(())
}

/// Main imposition function
pub async fn impose(documents: &[Document], options: &ImpositionOptions) -> Result<Document> {
    options.validate()?;

    let documents = documents.to_vec();
    let options = options.clone();

    tokio::task::spawn_blocking(move || impose_sync(&documents, &options)).await?
}

fn impose_sync(documents: &[Document], options: &ImpositionOptions) -> Result<Document> {
    // Merge all input documents into a single source
    let mut merged = merge_documents(documents)?;

    // Add flyleaves
    if options.front_flyleaves > 0 || options.back_flyleaves > 0 {
        merged = add_flyleaves(merged, options.front_flyleaves, options.back_flyleaves)?;
    }

    // Get total page count
    let total_pages = merged.get_pages().len();
    if total_pages == 0 {
        return Err(ImposeError::NoPages);
    }

    // Calculate imposition based on binding type
    match options.binding_type {
        BindingType::Signature | BindingType::CaseBinding => impose_signature(&merged, options),
        BindingType::PerfectBinding => impose_perfect_binding(&merged, options),
        BindingType::SideStitch | BindingType::Spiral => impose_simple_binding(&merged, options),
    }
}

fn merge_documents(documents: &[Document]) -> Result<Document> {
    if documents.is_empty() {
        return Err(ImposeError::NoPages);
    }

    if documents.len() == 1 {
        return Ok(documents[0].clone());
    }

    // For now, just clone the first document
    // A full implementation would properly merge all pages with resource dictionaries, fonts, etc.
    Ok(documents[0].clone())
}

fn add_flyleaves(mut doc: Document, front: usize, back: usize) -> Result<Document> {
    if front == 0 && back == 0 {
        return Ok(doc);
    }

    // Get existing page information
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Ok(doc);
    }

    let first_page_id = *pages.values().next().unwrap();
    let page_dict = doc.get_dictionary(first_page_id)?;

    let media_box = match page_dict.get(b"MediaBox")? {
        Object::Array(arr) => arr.clone(),
        _ => return Err(ImposeError::Config("MediaBox is not an array".to_string())),
    };

    // Get the pages root
    let catalog_id = doc.trailer.get(b"Root")?.as_reference()?;
    let catalog = doc.get_dictionary(catalog_id)?;
    let pages_id = catalog.get(b"Pages")?.as_reference()?;

    // Get existing page references - clone immediately to release borrow
    let kids = {
        let pages_dict = doc.get_dictionary(pages_id)?;
        if let Ok(Object::Array(arr)) = pages_dict.get(b"Kids") {
            arr.clone()
        } else {
            return Err(ImposeError::Config(
                "Pages Kids array not found".to_string(),
            ));
        }
    };

    // Create front flyleaves and insert at beginning
    let mut front_pages = Vec::new();
    for _ in 0..front {
        let blank_page_id = create_blank_page(&mut doc, &media_box, pages_id)?;
        front_pages.push(Object::Reference(blank_page_id));
    }

    // Create back flyleaves
    let mut back_pages = Vec::new();
    for _ in 0..back {
        let blank_page_id = create_blank_page(&mut doc, &media_box, pages_id)?;
        back_pages.push(Object::Reference(blank_page_id));
    }

    // Rebuild kids array: front flyleaves + existing pages + back flyleaves
    let mut new_kids = Vec::new();
    new_kids.extend(front_pages);
    new_kids.extend(kids);
    new_kids.extend(back_pages);

    // Update the pages dictionary with new kids array and count
    let count = new_kids.len() as i64;
    let pages_dict = doc.get_dictionary(pages_id)?;
    let mut updated_pages_dict = pages_dict.clone();
    updated_pages_dict.set("Count", Object::Integer(count));
    updated_pages_dict.set("Kids", Object::Array(new_kids));

    doc.objects
        .insert(pages_id, Object::Dictionary(updated_pages_dict));

    Ok(doc)
}

fn create_blank_page(
    doc: &mut Document,
    media_box: &[Object],
    parent_id: lopdf::ObjectId,
) -> Result<lopdf::ObjectId> {
    // Create an empty content stream
    let content_stream = Stream::new(Dictionary::new(), Vec::new());
    let content_id = doc.add_object(content_stream);

    // Create the page dictionary
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_id));
    page_dict.set("MediaBox", Object::Array(media_box.to_vec()));
    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(Dictionary::new()));

    let page_id = doc.add_object(page_dict);
    Ok(page_id)
}

fn impose_signature(doc: &Document, options: &ImpositionOptions) -> Result<Document> {
    let pages = doc.get_pages();
    let page_ids: Vec<_> = pages.values().copied().collect(); // Vec<ObjectId>
    let total_pages = page_ids.len();

    let pages_per_sig = options.page_arrangement.pages_per_signature();

    // Pad to multiple of pages_per_signature
    let padded_count = ((total_pages + pages_per_sig - 1) / pages_per_sig) * pages_per_sig;

    // Calculate signature layout
    let page_order = calculate_signature_order(padded_count, pages_per_sig);

    // Create output document with imposed pages
    impose_with_order(doc, &page_ids, &page_order, options)
}

/// Calculate the correct page order for signature binding using traditional bookbinding layout.
///
/// Based on Wikipedia diagrams for traditional bookbinding:
///
/// **Folio (4 pages, 1 fold):**
/// - Side A: [4, 1] (left=4, right=1)
/// - Side B: [2, 3] (left=2, right=3)
/// - No rotation needed
///
/// **Quarto (8 pages, 2 folds):**
/// - Side A: Top [5↓, 4↓], Bottom [8, 1]
/// - Side B: Top [6↓, 3↓], Bottom [7, 2]
/// - Top row rotated 180°
///
/// **Octavo (16 pages, 3 folds):**
/// - Side A: Top [5↓, 12↓, 9↓, 8↓], Bottom [4, 13, 16, 1]
/// - Side B: Top [6↓, 11↓, 10↓, 7↓], Bottom [3, 14, 15, 2]
/// - Top row rotated 180°
///
/// Returns pages grouped by output sheet side, ready for grid placement.
/// For quarto: [side_a_top_left, side_a_top_right, side_a_bottom_left, side_a_bottom_right,
///              side_b_top_left, side_b_top_right, side_b_bottom_left, side_b_bottom_right]
pub fn calculate_signature_order(total_pages: usize, pages_per_sig: usize) -> Vec<Option<usize>> {
    let num_signatures = total_pages / pages_per_sig;
    let mut order = Vec::with_capacity(total_pages);

    for sig_num in 0..num_signatures {
        let sig_start = sig_num * pages_per_sig;

        let sig_order: Vec<usize> = match pages_per_sig {
            4 => {
                // Folio: Side A [4, 1], Side B [2, 3]
                // No mirroring needed - folio is a simple 2-up with one fold
                // Positions: [left, right] for each side
                vec![
                    sig_start + 3, // Side A left: page 4
                    sig_start + 0, // Side A right: page 1
                    sig_start + 1, // Side B left: page 2
                    sig_start + 2, // Side B right: page 3
                ]
            }
            8 => {
                // Quarto: Side A top [5,4], bottom [8,1]; Side B top [3,6], bottom [2,7]
                // Grid order: top-left, top-right, bottom-left, bottom-right
                // Side B is horizontally mirrored because the paper flips for duplex printing
                vec![
                    sig_start + 4, // Side A top-left: page 5
                    sig_start + 3, // Side A top-right: page 4
                    sig_start + 7, // Side A bottom-left: page 8
                    sig_start + 0, // Side A bottom-right: page 1
                    sig_start + 2, // Side B top-left: page 3 (mirrored)
                    sig_start + 5, // Side B top-right: page 6 (mirrored)
                    sig_start + 1, // Side B bottom-left: page 2 (mirrored)
                    sig_start + 6, // Side B bottom-right: page 7 (mirrored)
                ]
            }
            16 => {
                // Octavo: 4 cols × 2 rows per side
                // Side A: Top [5,12,9,8], Bottom [4,13,16,1]
                // Side B: Top [7,10,11,6], Bottom [1,16,13,4] - mirrored for duplex
                // Wait, that's wrong. Side B has different pages.
                // Side B (per Wikipedia): Top [6,11,10,7], Bottom [3,14,15,2]
                // Mirrored: Top [7,10,11,6], Bottom [2,15,14,3]
                vec![
                    // Side A - top row (left to right)
                    sig_start + 4,  // page 5
                    sig_start + 11, // page 12
                    sig_start + 8,  // page 9
                    sig_start + 7,  // page 8
                    // Side A - bottom row (left to right)
                    sig_start + 3,  // page 4
                    sig_start + 12, // page 13
                    sig_start + 15, // page 16
                    sig_start + 0,  // page 1
                    // Side B - top row (mirrored: right to left becomes left to right)
                    sig_start + 6,  // page 7
                    sig_start + 9,  // page 10
                    sig_start + 10, // page 11
                    sig_start + 5,  // page 6
                    // Side B - bottom row (mirrored)
                    sig_start + 1,  // page 2
                    sig_start + 14, // page 15
                    sig_start + 13, // page 14
                    sig_start + 2,  // page 3
                ]
            }
            _ => {
                // Generic algorithm for custom page counts
                // Uses the traditional saddle-stitch pattern
                let sheets = pages_per_sig / 4;
                let mut pages = Vec::with_capacity(pages_per_sig);
                for i in 0..sheets {
                    let last = pages_per_sig - 1 - (2 * i);
                    let first = 2 * i;
                    pages.push(sig_start + last);
                    pages.push(sig_start + first);
                    pages.push(sig_start + first + 1);
                    pages.push(sig_start + last - 1);
                }
                pages
            }
        };

        for page_idx in sig_order {
            if page_idx < total_pages {
                order.push(Some(page_idx));
            } else {
                order.push(None);
            }
        }
    }

    order
}

fn impose_perfect_binding(doc: &Document, options: &ImpositionOptions) -> Result<Document> {
    // Perfect binding: pages are simply arranged in 2-up format, no folding
    let pages = doc.get_pages();
    let page_ids: Vec<_> = pages.values().copied().collect();

    // Simple 2-up order
    let mut order = Vec::new();
    for i in 0..page_ids.len() {
        order.push(Some(i));
    }
    if order.len() % 2 == 1 {
        order.push(None); // blank page
    }

    impose_with_order(doc, &page_ids, &order, options)
}

fn impose_simple_binding(doc: &Document, options: &ImpositionOptions) -> Result<Document> {
    // Side stitch and spiral: simple 2-up layout
    impose_perfect_binding(doc, options)
}

fn impose_with_order(
    doc: &Document,
    page_ids: &[lopdf::ObjectId],
    page_order: &[Option<usize>],
    options: &ImpositionOptions,
) -> Result<Document> {
    let mut output = Document::with_version("1.7");

    let (output_width, output_height) = options
        .output_paper_size
        .dimensions_with_orientation(options.output_orientation);
    let output_width_pt = mm_to_pt(output_width);
    let output_height_pt = mm_to_pt(output_height);

    // Create page tree root ID
    let pages_id = output.new_object_id();

    let mut page_refs = Vec::new();

    // Determine chunking based on binding type
    match options.binding_type {
        BindingType::Signature | BindingType::CaseBinding => {
            // Signature binding: all pages of a signature go on one sheet (2 sides)
            let pages_per_sig = options.page_arrangement.pages_per_signature();
            let pages_per_side = pages_per_sig / 2;

            for chunk in page_order.chunks(pages_per_sig) {
                // Process front of sheet
                if !chunk.is_empty() {
                    let front_pages_end = pages_per_side.min(chunk.len());
                    let front_page = create_imposed_page(
                        &mut output,
                        doc,
                        page_ids,
                        &chunk[0..front_pages_end],
                        output_width_pt,
                        output_height_pt,
                        pages_id,
                        options,
                        true, // is_front
                    )?;
                    page_refs.push(Object::Reference(front_page));
                }

                // Process back of sheet
                if chunk.len() > pages_per_side {
                    let back_page = create_imposed_page(
                        &mut output,
                        doc,
                        page_ids,
                        &chunk[pages_per_side..],
                        output_width_pt,
                        output_height_pt,
                        pages_id,
                        options,
                        false, // is_back
                    )?;
                    page_refs.push(Object::Reference(back_page));
                }
            }
        }
        BindingType::PerfectBinding | BindingType::SideStitch | BindingType::Spiral => {
            // Simple 2-up binding: 2 pages per output page (side by side)
            // Each output page is one side of a sheet
            for chunk in page_order.chunks(2) {
                if !chunk.is_empty() {
                    let page = create_imposed_page(
                        &mut output,
                        doc,
                        page_ids,
                        chunk,
                        output_width_pt,
                        output_height_pt,
                        pages_id,
                        options,
                        true, // Doesn't matter for 2-up, no flipping needed
                    )?;
                    page_refs.push(Object::Reference(page));
                }
            }
        }
    }

    // Create pages tree
    let count = page_refs.len() as i64;
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(page_refs)),
        ("Count", Object::Integer(count)),
    ]);
    output
        .objects
        .insert(pages_id, Object::Dictionary(pages_dict));

    // Create catalog
    let catalog_id = output.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));

    output.trailer.set("Root", catalog_id);

    Ok(output)
}

fn create_imposed_page(
    output: &mut Document,
    source: &Document,
    page_ids: &[lopdf::ObjectId],
    page_chunk: &[Option<usize>],
    output_width_pt: f32,
    output_height_pt: f32,
    parent_pages_id: lopdf::ObjectId,
    options: &ImpositionOptions,
    _is_front: bool,
) -> Result<lopdf::ObjectId> {
    // Create new output page
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_pages_id));
    page_dict.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(output_width_pt),
            Object::Real(output_height_pt),
        ]),
    );

    // Build content stream that places source pages
    let mut content_ops = Vec::new();
    let mut resources = Dictionary::new();
    let mut xobjects = Dictionary::new();

    // Determine layout based on number of pages
    // For signature binding with multiple pages per side, arrange in grid
    let num_pages = page_chunk.len();

    let (cols, rows) = match num_pages {
        1 => (1, 1),
        2 => (2, 1), // Folio: 2 cols × 1 row
        4 => (2, 2), // Quarto: 2 cols × 2 rows
        8 => (4, 2), // Octavo: 4 cols × 2 rows
        _ => {
            // For other sizes, prefer wider layouts
            let cols = if num_pages >= 8 { 4 } else { 2 };
            let rows = (num_pages + cols - 1) / cols;
            (cols, rows)
        }
    };

    // Convert sheet margins from mm to pt (printer-safe area)
    let sheet_left_pt = mm_to_pt(options.margins.sheet.left_mm);
    let sheet_right_pt = mm_to_pt(options.margins.sheet.right_mm);
    let sheet_top_pt = mm_to_pt(options.margins.sheet.top_mm);
    let sheet_bottom_pt = mm_to_pt(options.margins.sheet.bottom_mm);

    // Convert leaf margins from mm to pt (per-page margins within cells)
    let leaf_top_pt = mm_to_pt(options.margins.leaf.top_mm);
    let leaf_bottom_pt = mm_to_pt(options.margins.leaf.bottom_mm);
    let leaf_fore_edge_pt = mm_to_pt(options.margins.leaf.fore_edge_mm);
    let leaf_spine_pt = mm_to_pt(options.margins.leaf.spine_mm);

    // Leaf area: the region inside sheet margins where the signature content goes
    // This is where crop marks will be placed (corners of leaf area)
    let leaf_left = sheet_left_pt;
    let leaf_bottom = sheet_bottom_pt;
    let leaf_width = output_width_pt - sheet_left_pt - sheet_right_pt;
    let leaf_height = output_height_pt - sheet_top_pt - sheet_bottom_pt;
    let leaf_right = leaf_left + leaf_width;
    let leaf_top = leaf_bottom + leaf_height;

    // Cell dimensions (each cell holds one page of the signature)
    let cell_width = leaf_width / cols as f32;
    let cell_height = leaf_height / rows as f32;

    for (pos, page_idx_opt) in page_chunk.iter().enumerate() {
        if let Some(page_idx) = page_idx_opt {
            if *page_idx < page_ids.len() {
                let source_page_id = page_ids[*page_idx];

                // Get source page dimensions
                if let Ok(source_dict) = source.get_dictionary(source_page_id) {
                    let media_box = source_dict
                        .get(b"MediaBox")
                        .and_then(|obj| obj.as_array())
                        .ok();

                    if let Some(mb) = media_box {
                        let src_width = extract_number(&mb[2]).unwrap_or(612.0);
                        let src_height = extract_number(&mb[3]).unwrap_or(792.0);

                        // Calculate grid position
                        // Pages are provided in row-major order: row 0 left-to-right, then row 1, etc.
                        let col = pos % cols;
                        let row = pos / cols;

                        // Calculate cell origin (bottom-left of cell)
                        let cell_x = leaf_left + (col as f32 * cell_width);
                        let cell_y = leaf_bottom + ((rows - row - 1) as f32 * cell_height);

                        // Calculate margins for this cell based on its position and arrangement
                        // After folding, the spine is at the center of the sheet
                        //
                        // Folio (2 cols): spine between col 0 and 1
                        //   Col 0: fore-edge left, spine right
                        //   Col 1: spine left, fore-edge right
                        //
                        // Quarto (2 cols × 2 rows): same horizontal as folio
                        //   Top row is rotated 180°, so margins flip vertically for those cells
                        //
                        // Octavo (4 cols × 2 rows): center cut between cols 1 and 2
                        //   After cutting, each half is like a quarto
                        //   Left half (cols 0,1): spine between cols 0 and 1
                        //   Right half (cols 2,3): spine between cols 2 and 3
                        //   Col 0: fore-edge left, spine right
                        //   Col 1: spine left, fore-edge right (outer edge of left booklet)
                        //   Col 2: fore-edge left (outer edge of right booklet), spine right
                        //   Col 3: spine left, fore-edge right
                        //
                        // Note: cols 1 and 2 have fore-edge toward the center cut

                        let (margin_left, margin_right) = match cols {
                            2 => {
                                // Folio or Quarto: spine in center
                                if col == 0 {
                                    (leaf_fore_edge_pt, leaf_spine_pt)
                                } else {
                                    (leaf_spine_pt, leaf_fore_edge_pt)
                                }
                            }
                            4 => {
                                // Octavo: two spines, cut in center
                                match col {
                                    0 => (leaf_fore_edge_pt, leaf_spine_pt), // left booklet, left page
                                    1 => (leaf_spine_pt, leaf_fore_edge_pt), // left booklet, right page (fore-edge at cut)
                                    2 => (leaf_fore_edge_pt, leaf_spine_pt), // right booklet, left page (fore-edge at cut)
                                    3 => (leaf_spine_pt, leaf_fore_edge_pt), // right booklet, right page
                                    _ => (leaf_fore_edge_pt, leaf_fore_edge_pt),
                                }
                            }
                            _ => {
                                // Generic: use average margins
                                let avg = (leaf_fore_edge_pt + leaf_spine_pt) / 2.0;
                                (avg, avg)
                            }
                        };

                        // Vertical margins: top/bottom of the final page
                        // For rotated pages (top row in quarto/octavo), the margins swap
                        let needs_rotation = match options.page_arrangement {
                            PageArrangement::Folio => false,
                            PageArrangement::Quarto | PageArrangement::Octavo => row == 0,
                            PageArrangement::Custom { .. } => false,
                        };

                        let (margin_bottom, margin_top) = if needs_rotation {
                            // Page will be rotated 180°, so top becomes bottom
                            (leaf_top_pt, leaf_bottom_pt)
                        } else {
                            (leaf_bottom_pt, leaf_top_pt)
                        };

                        // Content area within this cell
                        let cell_content_left = cell_x + margin_left;
                        let cell_content_bottom = cell_y + margin_bottom;
                        let cell_content_width = cell_width - margin_left - margin_right;
                        let cell_content_height = cell_height - margin_top - margin_bottom;

                        // Calculate scale to fit within content area
                        let scale = calculate_scale(
                            src_width,
                            src_height,
                            cell_content_width,
                            cell_content_height,
                            options.scaling_mode,
                        );

                        let scaled_width = src_width * scale;
                        let scaled_height = src_height * scale;

                        // Center the scaled page within its content area
                        let x_pos = cell_content_left + (cell_content_width - scaled_width) / 2.0;
                        let y_pos =
                            cell_content_bottom + (cell_content_height - scaled_height) / 2.0;

                        // Create XObject from source page
                        let xobject_name = format!("P{}", pos);
                        let mut cache = HashMap::new();
                        let xobject_id =
                            create_page_xobject(output, source, source_page_id, &mut cache)?;
                        xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                        // Add transformation matrix and XObject reference to content stream
                        if needs_rotation {
                            // For 180° rotation, we need to:
                            // 1. Translate to rotation point (center of page)
                            // 2. Rotate 180°
                            // 3. Scale
                            // Matrix for 180° rotation is: [-1 0 0 -1 tx ty]
                            // We want to rotate around the center of the scaled page
                            let rot_x = x_pos + scaled_width;
                            let rot_y = y_pos + scaled_height;

                            content_ops.push(format!(
                                "q {} 0 0 {} {} {} cm /{} Do Q\n",
                                -scale, -scale, rot_x, rot_y, xobject_name
                            ));
                        } else {
                            content_ops.push(format!(
                                "q {} 0 0 {} {} {} cm /{} Do Q\n",
                                scale, scale, x_pos, y_pos, xobject_name
                            ));
                        }
                    }
                }
            }
        }
    }

    // Generate printer's marks if any are enabled
    // Marks are per-leaf (the entire signature area), not per-page
    let has_marks = options.marks.fold_lines
        || options.marks.cut_lines
        || options.marks.crop_marks
        || options.marks.registration_marks
        || options.marks.sewing_marks
        || options.marks.spine_marks;

    if has_marks {
        let marks_config = MarksConfig {
            cols,
            rows,
            cell_width,
            cell_height,
            leaf_left,
            leaf_bottom,
            leaf_right,
            leaf_top,
        };
        let marks_content = generate_marks(&options.marks, &marks_config);
        content_ops.push(marks_content);
    }

    // Set up resources dictionary
    resources.set("XObject", Object::Dictionary(xobjects));

    // Create content stream
    let content = content_ops.join("");
    let content_id = output.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(resources));

    // Add page to document
    let page_id = output.add_object(page_dict);
    Ok(page_id)
}

/// Create an XObject from a source page by copying all its resources
fn create_page_xobject(
    output: &mut Document,
    source: &Document,
    page_id: lopdf::ObjectId,
    cache: &mut HashMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId> {
    let page_dict = source.get_dictionary(page_id)?;

    // Get page dimensions
    let media_box = page_dict
        .get(b"MediaBox")
        .and_then(|obj| obj.as_array())
        .ok()
        .cloned()
        .unwrap_or_else(|| {
            vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(612),
                Object::Integer(792),
            ]
        });

    // Get page content
    let content_data = get_page_content(source, page_dict)?;

    // Create XObject dictionary
    let mut xobject_dict = Dictionary::new();
    xobject_dict.set("Type", Object::Name(b"XObject".to_vec()));
    xobject_dict.set("Subtype", Object::Name(b"Form".to_vec()));
    xobject_dict.set("BBox", Object::Array(media_box));
    xobject_dict.set("FormType", Object::Integer(1));

    // Copy resources if present, using a cache to avoid duplicating objects
    if let Ok(resources) = page_dict.get(b"Resources") {
        xobject_dict.set(
            "Resources",
            copy_object_deep(output, source, resources, cache)?,
        );
    }

    // Create XObject with content stream
    let xobject_id = output.add_object(Stream::new(xobject_dict, content_data));

    Ok(xobject_id)
}

/// Get the content stream data from a page
fn get_page_content(doc: &Document, page_dict: &Dictionary) -> Result<Vec<u8>> {
    let contents = page_dict.get(b"Contents")?;

    match contents {
        Object::Reference(id) => {
            // Single content stream
            if let Ok(stream) = doc.get_object(*id)?.as_stream() {
                // Try decompressed first, but fall back to raw content if no compression
                match stream.decompressed_content() {
                    Ok(content) => Ok(content),
                    Err(_) => Ok(stream.content.clone()),
                }
            } else {
                Ok(Vec::new())
            }
        }
        Object::Array(arr) => {
            // Multiple content streams - concatenate them
            let mut result = Vec::new();
            for obj in arr {
                if let Object::Reference(id) = obj {
                    if let Ok(stream) = doc.get_object(*id)?.as_stream() {
                        // Try decompressed first, but fall back to raw content if no compression
                        let content = match stream.decompressed_content() {
                            Ok(c) => c,
                            Err(_) => stream.content.clone(),
                        };
                        result.extend_from_slice(&content);
                        result.push(b'\n');
                    }
                }
            }
            Ok(result)
        }
        _ => Ok(Vec::new()),
    }
}

/// Deep copy an object from source to output document, following references
/// Uses a cache to avoid copying the same object multiple times
fn copy_object_deep(
    output: &mut Document,
    source: &Document,
    obj: &Object,
    cache: &mut HashMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<Object> {
    match obj {
        Object::Reference(id) => {
            // Check if we've already copied this object
            if let Some(&new_id) = cache.get(id) {
                return Ok(Object::Reference(new_id));
            }

            // Get the referenced object and copy it
            let referenced = source.get_object(*id)?;
            let copied = copy_object_deep(output, source, referenced, cache)?;

            // Add the copied object to output and cache the mapping
            let new_id = output.add_object(copied);
            cache.insert(*id, new_id);

            Ok(Object::Reference(new_id))
        }
        Object::Dictionary(dict) => {
            let mut new_dict = Dictionary::new();
            for (key, value) in dict.iter() {
                new_dict.set(key.clone(), copy_object_deep(output, source, value, cache)?);
            }
            Ok(Object::Dictionary(new_dict))
        }
        Object::Array(arr) => {
            let mut new_arr = Vec::new();
            for item in arr {
                new_arr.push(copy_object_deep(output, source, item, cache)?);
            }
            Ok(Object::Array(new_arr))
        }
        Object::Stream(stream) => {
            let mut new_dict = Dictionary::new();
            for (key, value) in stream.dict.iter() {
                // Recursively copy dictionary entries, which may contain indirect references
                new_dict.set(key.clone(), copy_object_deep(output, source, value, cache)?);
            }
            Ok(Object::Stream(Stream {
                dict: new_dict,
                content: stream.content.clone(),
                allows_compression: stream.allows_compression,
                start_position: None,
            }))
        }
        // For primitive types, just clone them
        _ => Ok(obj.clone()),
    }
}

fn mm_to_pt(mm: f32) -> f32 {
    mm * 2.83465
}

fn extract_number(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

fn calculate_scale(src_w: f32, src_h: f32, target_w: f32, target_h: f32, mode: ScalingMode) -> f32 {
    match mode {
        ScalingMode::Fit => {
            let scale_w = target_w / src_w;
            let scale_h = target_h / src_h;
            scale_w.min(scale_h)
        }
        ScalingMode::Fill => {
            let scale_w = target_w / src_w;
            let scale_h = target_h / src_h;
            scale_w.max(scale_h)
        }
        ScalingMode::None => 1.0,
        ScalingMode::Stretch => {
            // Use width scaling (aspect ratio ignored)
            target_w / src_w
        }
    }
}
