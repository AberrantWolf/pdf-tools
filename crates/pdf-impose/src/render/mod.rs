//! PDF rendering modules for imposition
//!
//! This module handles all PDF-specific operations:
//! - Creating XObjects from source pages
//! - Building imposed output pages
//! - Generating transformation matrices
//! - Deep copying PDF objects

mod page;
mod xobject;

pub use page::*;
pub use xobject::{copy_object_deep, create_page_xobject, get_page_dimensions};
