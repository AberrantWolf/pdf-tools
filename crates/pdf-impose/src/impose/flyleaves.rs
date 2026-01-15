//! Flyleaf handling for imposition

use crate::types::*;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

/// Pages per leaf (front and back)
const PAGES_PER_LEAF: usize = 2;

/// Add flyleaves (blank pages) to front and back of document
pub(crate) fn add_flyleaves(mut doc: Document, front: usize, back: usize) -> Result<Document> {
    if front == 0 && back == 0 {
        return Ok(doc);
    }

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

    let catalog_id = doc.trailer.get(b"Root")?.as_reference()?;
    let catalog = doc.get_dictionary(catalog_id)?;
    let pages_id = catalog.get(b"Pages")?.as_reference()?;

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

    let mut front_pages = Vec::new();
    for _ in 0..(front * PAGES_PER_LEAF) {
        let blank_page_id = create_blank_page(&mut doc, &media_box, pages_id)?;
        front_pages.push(Object::Reference(blank_page_id));
    }

    let mut back_pages = Vec::new();
    for _ in 0..(back * PAGES_PER_LEAF) {
        let blank_page_id = create_blank_page(&mut doc, &media_box, pages_id)?;
        back_pages.push(Object::Reference(blank_page_id));
    }

    let mut new_kids = Vec::new();
    new_kids.extend(front_pages);
    new_kids.extend(kids);
    new_kids.extend(back_pages);

    let count = new_kids.len() as i64;
    let pages_dict = doc.get_dictionary(pages_id)?;
    let mut updated_pages_dict = pages_dict.clone();
    updated_pages_dict.set("Count", Object::Integer(count));
    updated_pages_dict.set("Kids", Object::Array(new_kids));

    doc.objects
        .insert(pages_id, Object::Dictionary(updated_pages_dict));

    Ok(doc)
}

/// Create a blank page with the given media box
fn create_blank_page(
    doc: &mut Document,
    media_box: &[Object],
    parent_id: ObjectId,
) -> Result<ObjectId> {
    let content_stream = Stream::new(Dictionary::new(), Vec::new());
    let content_id = doc.add_object(content_stream);

    let mut page_dict = Dictionary::new();
    page_dict.set("Type", Object::Name(b"Page".to_vec()));
    page_dict.set("Parent", Object::Reference(parent_id));
    page_dict.set("MediaBox", Object::Array(media_box.to_vec()));
    page_dict.set("Contents", Object::Reference(content_id));
    page_dict.set("Resources", Object::Dictionary(Dictionary::new()));

    let page_id = doc.add_object(page_dict);
    Ok(page_id)
}
