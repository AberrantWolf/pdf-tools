use crate::options::ImpositionOptions;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, Stream};
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
    // Get page size from first page
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

    // Create blank pages with same dimensions
    for _ in 0..front {
        create_blank_page(&mut doc, &media_box)?;
    }

    // Add back flyleaves at end (would need to append to page tree)
    for _ in 0..back {
        create_blank_page(&mut doc, &media_box)?;
    }

    Ok(doc)
}

fn create_blank_page(doc: &mut Document, media_box: &[Object]) -> Result<()> {
    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("MediaBox", Object::Array(media_box.to_vec()));
    page_dict.set("Contents", doc.new_object_id());

    let _ = doc.new_object_id();
    // Would need to properly insert into page tree

    Ok(())
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

fn calculate_signature_order(total_pages: usize, pages_per_sig: usize) -> Vec<Option<usize>> {
    let num_signatures = total_pages / pages_per_sig;
    let sheets_per_sig = pages_per_sig / 4;

    let mut order = Vec::with_capacity(total_pages);

    for sig_num in 0..num_signatures {
        let sig_start = sig_num * pages_per_sig;

        // For each sheet in the signature
        for sheet_num in 0..sheets_per_sig {
            // Front of sheet (outer pages, right to left)
            let outer_right = sig_start + (pages_per_sig - 1) - (sheet_num * 2);
            let outer_left = sig_start + (sheet_num * 2);

            order.push(Some(outer_right));
            order.push(Some(outer_left));

            // Back of sheet (inner pages, left to right)
            let inner_left = sig_start + (sheet_num * 2) + 1;
            let inner_right = sig_start + (pages_per_sig - 2) - (sheet_num * 2);

            order.push(Some(inner_left));
            order.push(Some(inner_right));
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

    let (output_width, output_height) = options.output_paper_size.dimensions_mm();
    let output_width_pt = mm_to_pt(output_width);
    let output_height_pt = mm_to_pt(output_height);

    // Pages per sheet (2 for most bindings)
    let pages_per_sheet = 2;

    // Create page tree root ID
    let pages_id = output.new_object_id();

    let mut page_refs = Vec::new();

    // Process pages in chunks
    for chunk in page_order.chunks(pages_per_sheet) {
        // Create new output page
        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name(b"Page".to_vec()));
        page_dict.set("Parent", Object::Reference(pages_id));
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

        for (pos, page_idx_opt) in chunk.iter().enumerate() {
            if let Some(page_idx) = page_idx_opt {
                if *page_idx < page_ids.len() {
                    let source_page_id = page_ids[*page_idx];

                    // Get source page dimensions
                    if let Ok(source_dict) = doc.get_dictionary(source_page_id) {
                        let media_box = source_dict
                            .get(b"MediaBox")
                            .and_then(|obj| obj.as_array())
                            .ok();

                        if let Some(mb) = media_box {
                            let src_width = extract_number(&mb[2]).unwrap_or(612.0);
                            let src_height = extract_number(&mb[3]).unwrap_or(792.0);

                            // Calculate position and scale
                            let target_width = output_width_pt / pages_per_sheet as f32;
                            let x_offset = pos as f32 * target_width;

                            let scale = calculate_scale(
                                src_width,
                                src_height,
                                target_width,
                                output_height_pt,
                                options.scaling_mode,
                            );

                            // Center the scaled page within its target area
                            let scaled_width = src_width * scale;
                            let scaled_height = src_height * scale;
                            let x_center = x_offset + (target_width - scaled_width) / 2.0;
                            let y_center = (output_height_pt - scaled_height) / 2.0;

                            // Create XObject from source page
                            let xobject_name = format!("P{}", pos);
                            let xobject_id = create_page_xobject(&mut output, doc, source_page_id)?;
                            xobjects.set(xobject_name.as_bytes(), Object::Reference(xobject_id));

                            // Add transformation matrix and XObject reference to content stream
                            content_ops.push(format!(
                                "q {} 0 0 {} {} {} cm /{} Do Q\n",
                                scale, scale, x_center, y_center, xobject_name
                            ));
                        }
                    }
                }
            }
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
        page_refs.push(Object::Reference(page_id));
    }

    // Create pages tree
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(page_refs)),
        (
            "Count",
            Object::Integer(page_order.chunks(pages_per_sheet).len() as i64),
        ),
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

/// Create an XObject from a source page by copying all its resources
fn create_page_xobject(
    output: &mut Document,
    source: &Document,
    page_id: lopdf::ObjectId,
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

    // Copy resources if present
    if let Ok(resources) = page_dict.get(b"Resources") {
        xobject_dict.set("Resources", copy_object_deep(output, source, resources)?);
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
                Ok(stream.decompressed_content()?)
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
                        result.extend_from_slice(&stream.decompressed_content()?);
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
fn copy_object_deep(output: &mut Document, source: &Document, obj: &Object) -> Result<Object> {
    match obj {
        Object::Reference(id) => {
            // Get the referenced object and copy it
            let referenced = source.get_object(*id)?;
            let copied = copy_object_deep(output, source, referenced)?;
            // Add the copied object to output and return a reference to it
            let new_id = output.add_object(copied);
            Ok(Object::Reference(new_id))
        }
        Object::Dictionary(dict) => {
            let mut new_dict = Dictionary::new();
            for (key, value) in dict.iter() {
                new_dict.set(key.clone(), copy_object_deep(output, source, value)?);
            }
            Ok(Object::Dictionary(new_dict))
        }
        Object::Array(arr) => {
            let mut new_arr = Vec::new();
            for item in arr {
                new_arr.push(copy_object_deep(output, source, item)?);
            }
            Ok(Object::Array(new_arr))
        }
        Object::Stream(stream) => {
            let mut new_dict = Dictionary::new();
            for (key, value) in stream.dict.iter() {
                new_dict.set(key.clone(), copy_object_deep(output, source, value)?);
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
