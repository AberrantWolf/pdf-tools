//! PDF rendering modules for imposition
//!
//! This module handles all PDF-specific operations:
//! - Creating XObjects from source pages
//! - Building imposed output pages
//! - Generating transformation matrices

mod page;
mod xobject;

pub use page::*;
pub use xobject::*;
