//! Flyleaf handling for imposition
//!
//! Flyleaves are blank pages added to the front and back of a book.
//! Each flyleaf consists of 2 pages (front and back of one leaf).

use crate::constants::PAGES_PER_LEAF;
use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// Add flyleaves (blank pages) to front and back of document
///
/// # Arguments
/// * `doc` - The document to modify
/// * `front` - Number of flyleaves to add at the front
/// * `back` - Number of flyleaves to add at the back
pub(crate) fn add_flyleaves(mut doc: Document, front: usize, back: usize) -> Result<Document> {
    if front == 0 && back == 0 {
        return Ok(doc);
    }

    let pages = doc.get_pages();
    if pages.is_empty() {
        return Ok(doc);
    }

    // Get media box from first page
    let first_page_id = *pages.values().next().unwrap();
    let media_box = get_media_box(&doc, first_page_id)?;

    // Get pages tree
    let (pages_id, kids) = get_pages_tree(&doc)?;

    // Create blank pages
    let front_pages = create_blank_pages(&mut doc, &media_box, pages_id, front * PAGES_PER_LEAF)?;
    let back_pages = create_blank_pages(&mut doc, &media_box, pages_id, back * PAGES_PER_LEAF)?;

    // Build new kids array: front + existing + back
    let mut new_kids = Vec::with_capacity(front_pages.len() + kids.len() + back_pages.len());
    new_kids.extend(front_pages);
    new_kids.extend(kids);
    new_kids.extend(back_pages);

    // Update pages tree
    update_pages_tree(&mut doc, pages_id, new_kids)?;

    Ok(doc)
}

/// Get the MediaBox from a page
fn get_media_box(doc: &Document, page_id: ObjectId) -> Result<Vec<Object>> {
    let page_dict = doc.get_dictionary(page_id)?;

    match page_dict.get(b"MediaBox")? {
        Object::Array(arr) => Ok(arr.clone()),
        _ => Err(ImposeError::Config("MediaBox is not an array".to_string())),
    }
}

/// Get the pages tree (pages object ID and kids array)
fn get_pages_tree(doc: &Document) -> Result<(ObjectId, Vec<Object>)> {
    let catalog_id = doc.trailer.get(b"Root")?.as_reference()?;
    let catalog = doc.get_dictionary(catalog_id)?;
    let pages_id = catalog.get(b"Pages")?.as_reference()?;

    let pages_dict = doc.get_dictionary(pages_id)?;
    let kids = pages_dict
        .get(b"Kids")
        .and_then(|obj| obj.as_array())
        .map(|arr| arr.clone())
        .ok()
        .ok_or_else(|| ImposeError::Config("Pages Kids array not found".to_string()))?;

    Ok((pages_id, kids))
}

/// Create multiple blank pages
fn create_blank_pages(
    doc: &mut Document,
    media_box: &[Object],
    parent_id: ObjectId,
    count: usize,
) -> Result<Vec<Object>> {
    (0..count)
        .map(|_| {
            let page_id = create_blank_page(doc, media_box, parent_id)?;
            Ok(Object::Reference(page_id))
        })
        .collect()
}

/// Create a single blank page with the given media box
fn create_blank_page(
    doc: &mut Document,
    media_box: &[Object],
    parent_id: ObjectId,
) -> Result<ObjectId> {
    let content_id = doc.add_object(Stream::new(Dictionary::new(), Vec::new()));

    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_id));
    page_dict.set("MediaBox", Object::Array(media_box.to_vec()));
    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(Dictionary::new()));

    Ok(doc.add_object(page_dict))
}

/// Update the pages tree with new kids
fn update_pages_tree(doc: &mut Document, pages_id: ObjectId, new_kids: Vec<Object>) -> Result<()> {
    let pages_dict = doc.get_dictionary(pages_id)?;
    let mut updated = pages_dict.clone();

    updated.set("Count", Object::Integer(new_kids.len() as i64));
    updated.set("Kids", Object::Array(new_kids));

    doc.objects.insert(pages_id, Object::Dictionary(updated));
    Ok(())
}
