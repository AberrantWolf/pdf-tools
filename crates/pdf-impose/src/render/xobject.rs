//! XObject creation for imposition
//!
//! This module handles creating Form XObjects from source PDF pages,
//! which are then placed onto output pages with transformations.

use crate::types::Result;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::HashMap;

/// Create an XObject from a source page.
///
/// The XObject can then be placed multiple times on output pages
/// with different transformations.
///
/// # Arguments
/// * `output` - The output document to add the XObject to
/// * `source` - The source document containing the page
/// * `page_id` - The object ID of the source page
/// * `cache` - Cache to avoid copying the same object multiple times
pub fn create_page_xobject(
    output: &mut Document,
    source: &Document,
    page_id: ObjectId,
    cache: &mut HashMap<ObjectId, ObjectId>,
) -> Result<ObjectId> {
    let page_dict = source.get_dictionary(page_id)?;

    // Get page dimensions from MediaBox
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
        xobject_dict.set(
            "Resources",
            copy_object_deep(output, source, resources, cache)?,
        );
    }

    // Create XObject with content stream
    let xobject_id = output.add_object(Stream::new(xobject_dict, content_data));

    Ok(xobject_id)
}

/// Get the content stream data from a page.
fn get_page_content(doc: &Document, page_dict: &Dictionary) -> Result<Vec<u8>> {
    let contents = match page_dict.get(b"Contents") {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()), // No content = blank page
    };

    match contents {
        Object::Reference(id) => {
            // Single content stream
            if let Ok(stream) = doc.get_object(*id)?.as_stream() {
                // Try decompressed first, fall back to raw content
                match stream.decompressed_content() {
                    Ok(content) => Ok(content),
                    Err(_) => Ok(stream.content.clone()),
                }
            } else {
                Ok(Vec::new())
            }
        }
        Object::Array(arr) => {
            // Multiple content streams - concatenate
            let mut result = Vec::new();
            for obj in arr {
                if let Object::Reference(id) = obj {
                    if let Ok(stream) = doc.get_object(*id)?.as_stream() {
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

/// Deep copy an object from source to output document, following references.
///
/// Uses a cache to avoid copying the same object multiple times.
pub fn copy_object_deep(
    output: &mut Document,
    source: &Document,
    obj: &Object,
    cache: &mut HashMap<ObjectId, ObjectId>,
) -> Result<Object> {
    match obj {
        Object::Reference(id) => {
            // Check cache first
            if let Some(&new_id) = cache.get(id) {
                return Ok(Object::Reference(new_id));
            }

            // Get and copy the referenced object
            let referenced = source.get_object(*id)?;
            let copied = copy_object_deep(output, source, referenced, cache)?;

            // Add to output and cache
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
                new_dict.set(key.clone(), copy_object_deep(output, source, value, cache)?);
            }
            Ok(Object::Stream(Stream {
                dict: new_dict,
                content: stream.content.clone(),
                allows_compression: stream.allows_compression,
                start_position: None,
            }))
        }
        // Primitive types: just clone
        _ => Ok(obj.clone()),
    }
}

/// Extract numeric value from a PDF object
pub fn extract_number(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

/// Get source page dimensions (width, height) in points
pub fn get_page_dimensions(doc: &Document, page_id: ObjectId) -> Result<(f32, f32)> {
    let page_dict = doc.get_dictionary(page_id)?;

    let media_box = page_dict
        .get(b"MediaBox")
        .and_then(|obj| obj.as_array())
        .ok();

    if let Some(mb) = media_box {
        let width = extract_number(&mb[2]).unwrap_or(612.0);
        let height = extract_number(&mb[3]).unwrap_or(792.0);
        Ok((width, height))
    } else {
        Ok((612.0, 792.0)) // Default to US Letter
    }
}
