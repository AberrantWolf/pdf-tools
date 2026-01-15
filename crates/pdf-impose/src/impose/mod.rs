//! PDF imposition - arranging pages for bookbinding
//!
//! This module orchestrates the imposition process:
//! 1. Load and merge source documents
//! 2. Calculate signature layouts
//! 3. Place pages with margins and alignment
//! 4. Render to output PDF with printer's marks

mod flyleaves;
mod io;
mod sheet;
mod signature;
mod simple;

pub use io::{load_multiple_pdfs, load_pdf, save_pdf};

use crate::constants::mm_to_pt;
use crate::options::ImpositionOptions;
use crate::types::*;
use flyleaves::add_flyleaves;
use io::merge_documents;
use lopdf::{Document, ObjectId};

// =============================================================================
// Main Entry Point
// =============================================================================

/// Main imposition function
///
/// Takes source documents and options, returns an imposed output document.
pub async fn impose(documents: &[Document], options: &ImpositionOptions) -> Result<Document> {
    options.validate()?;

    let documents = documents.to_vec();
    let options = options.clone();

    tokio::task::spawn_blocking(move || impose_sync(&documents, &options)).await?
}

fn impose_sync(documents: &[Document], options: &ImpositionOptions) -> Result<Document> {
    // Merge all input documents into a single source
    let mut merged = merge_documents(documents)?;

    // Add flyleaves (each flyleaf = 1 leaf = 2 pages)
    if options.front_flyleaves > 0 || options.back_flyleaves > 0 {
        merged = add_flyleaves(merged, options.front_flyleaves, options.back_flyleaves)?;
    }

    // Get source page info
    let pages = merged.get_pages();
    let page_ids: Vec<ObjectId> = pages.values().copied().collect();
    let total_pages = page_ids.len();

    if total_pages == 0 {
        return Err(ImposeError::NoPages);
    }

    // Dispatch based on binding type
    if options.binding_type.uses_signatures() {
        signature::impose_signature_binding(&merged, &page_ids, options)
    } else {
        simple::impose_simple_binding(&merged, &page_ids, options)
    }
}

// =============================================================================
// Shared Utilities
// =============================================================================

/// Calculate output sheet dimensions in points
pub(crate) fn sheet_dimensions_pt(options: &ImpositionOptions) -> (f32, f32) {
    let (width_mm, height_mm) = options
        .output_paper_size
        .dimensions_with_orientation(options.output_orientation);
    (mm_to_pt(width_mm), mm_to_pt(height_mm))
}
