//! Layout calculation modules for imposition
//!
//! This module handles all the geometric calculations for page imposition:
//! - Signature slot ordering (which source page goes where)
//! - Grid layout (cell dimensions, fold/cut positions)
//! - Content placement (margins, alignment, scaling)

mod grid;
mod placement;
mod signature;
mod types;

pub use grid::*;
pub use placement::*;
pub use signature::*;
pub use types::*;
