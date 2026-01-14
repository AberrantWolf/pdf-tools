use crate::impose::impose;
use crate::options::ImpositionOptions;
use crate::types::*;
use lopdf::Document;

/// Generate a preview of the imposition
/// Returns a document with a limited number of sheets for preview
pub async fn generate_preview(
    documents: &[Document],
    options: &ImpositionOptions,
    max_sheets: usize,
) -> Result<Document> {
    // Create a modified options that limits output
    let preview_options = options.clone();

    // Calculate how many source pages we need for the preview
    let pages_per_sig = options.page_arrangement.pages_per_signature();
    let source_pages_needed = match options.binding_type {
        BindingType::Signature | BindingType::CaseBinding => {
            // Show max_sheets signatures (each signature is 1 sheet with 2 sides)
            max_sheets * pages_per_sig
        }
        BindingType::PerfectBinding | BindingType::SideStitch | BindingType::Spiral => {
            // Show max_sheets worth of pages (each sheet has 2 pages, 1 per side)
            max_sheets * 2
        }
    };

    // Create preview documents with limited pages
    let preview_docs = limit_document_pages(documents, source_pages_needed)?;

    // Impose with limited pages
    impose(&preview_docs, &preview_options).await
}

fn limit_document_pages(documents: &[Document], max_pages: usize) -> Result<Vec<Document>> {
    if documents.is_empty() {
        return Err(ImposeError::NoPages);
    }

    // Only process first document for now
    let doc = &documents[0];
    let pages = doc.get_pages();
    let total_pages = pages.len();

    if total_pages <= max_pages {
        // No need to limit
        return Ok(documents.to_vec());
    }

    // Create a new document with limited pages
    let mut limited_doc = Document::with_version(doc.version.as_str());

    // Copy relevant pages
    let page_ids: Vec<_> = pages.iter().take(max_pages).map(|(_, &id)| id).collect();

    // Create new pages tree
    let pages_tree_id = limited_doc.new_object_id();
    let mut kids = Vec::new();

    for &page_id in &page_ids {
        if let Ok(page_obj) = doc.get_object(page_id) {
            // Deep copy the page object
            let new_page_id = copy_object_to_doc(&mut limited_doc, doc, page_obj)?;
            kids.push(lopdf::Object::Reference(new_page_id));
        }
    }

    // Create pages dictionary
    let pages_dict = lopdf::Dictionary::from_iter(vec![
        ("Type", lopdf::Object::Name(b"Pages".to_vec())),
        ("Kids", lopdf::Object::Array(kids)),
        ("Count", lopdf::Object::Integer(page_ids.len() as i64)),
    ]);
    limited_doc
        .objects
        .insert(pages_tree_id, lopdf::Object::Dictionary(pages_dict));

    // Create catalog
    let catalog_id = limited_doc.add_object(lopdf::Dictionary::from_iter(vec![
        ("Type", lopdf::Object::Name(b"Catalog".to_vec())),
        ("Pages", lopdf::Object::Reference(pages_tree_id)),
    ]));

    limited_doc.trailer.set("Root", catalog_id);

    Ok(vec![limited_doc])
}

fn copy_object_to_doc(
    dest: &mut Document,
    source: &Document,
    obj: &lopdf::Object,
) -> Result<lopdf::ObjectId> {
    use std::collections::HashMap;

    let mut cache = HashMap::new();
    copy_object_deep_cached(dest, source, obj, &mut cache)
}

fn copy_object_deep_cached(
    dest: &mut Document,
    source: &Document,
    obj: &lopdf::Object,
    cache: &mut std::collections::HashMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId> {
    match obj {
        lopdf::Object::Reference(id) => {
            if let Some(&new_id) = cache.get(id) {
                Ok(new_id)
            } else {
                let referenced = source.get_object(*id)?;
                let new_id = copy_object_deep_cached(dest, source, referenced, cache)?;
                cache.insert(*id, new_id);
                Ok(new_id)
            }
        }
        lopdf::Object::Dictionary(dict) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, value) in dict.iter() {
                let new_value = match value {
                    lopdf::Object::Reference(id) => {
                        let new_id = if let Some(&cached_id) = cache.get(id) {
                            cached_id
                        } else {
                            let referenced = source.get_object(*id)?;
                            let new_id = copy_object_deep_cached(dest, source, referenced, cache)?;
                            cache.insert(*id, new_id);
                            new_id
                        };
                        lopdf::Object::Reference(new_id)
                    }
                    _ => value.clone(),
                };
                new_dict.set(key.clone(), new_value);
            }
            Ok(dest.add_object(new_dict))
        }
        lopdf::Object::Stream(stream) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, value) in stream.dict.iter() {
                let new_value = match value {
                    lopdf::Object::Reference(id) => {
                        let new_id = if let Some(&cached_id) = cache.get(id) {
                            cached_id
                        } else {
                            let referenced = source.get_object(*id)?;
                            let new_id = copy_object_deep_cached(dest, source, referenced, cache)?;
                            cache.insert(*id, new_id);
                            new_id
                        };
                        lopdf::Object::Reference(new_id)
                    }
                    _ => value.clone(),
                };
                new_dict.set(key.clone(), new_value);
            }
            let new_stream = lopdf::Stream {
                dict: new_dict,
                content: stream.content.clone(),
                allows_compression: stream.allows_compression,
                start_position: None,
            };
            Ok(dest.add_object(new_stream))
        }
        _ => Ok(dest.add_object(obj.clone())),
    }
}
