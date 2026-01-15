//! Statistics calculation for imposition
//!
//! Calculates output statistics without performing the actual imposition.

use crate::constants::PAGES_PER_LEAF;
use crate::options::ImpositionOptions;
use crate::types::*;
use lopdf::Document;

/// Calculate statistics for the imposition
///
/// Returns statistics about the output without generating the actual PDF.
pub fn calculate_statistics(
    documents: &[Document],
    options: &ImpositionOptions,
) -> Result<ImpositionStatistics> {
    // Count total source pages
    let mut source_pages: usize = documents.iter().map(|doc| doc.get_pages().len()).sum();

    // Add flyleaves (each flyleaf = 1 leaf = 2 pages)
    source_pages += (options.front_flyleaves + options.back_flyleaves) * PAGES_PER_LEAF;

    if source_pages == 0 {
        return Err(ImposeError::NoPages);
    }

    if options.binding_type.uses_signatures() {
        calculate_signature_stats(source_pages, options)
    } else {
        calculate_simple_stats(source_pages)
    }
}

/// Calculate statistics for signature binding
fn calculate_signature_stats(
    source_pages: usize,
    options: &ImpositionOptions,
) -> Result<ImpositionStatistics> {
    let pages_per_sig = options.page_arrangement.pages_per_signature();
    let sheets_per_sig = options.page_arrangement.sheets_per_signature();

    // Pad to multiple of pages_per_signature
    let padded_count = round_up_to_multiple(source_pages, pages_per_sig);
    let blank_pages_added = padded_count - source_pages;

    let num_signatures = padded_count / pages_per_sig;
    let total_sheets = num_signatures * sheets_per_sig;

    // Output pages (front and back of each sheet)
    let output_pages = total_sheets * 2;

    Ok(ImpositionStatistics {
        source_pages,
        output_sheets: total_sheets,
        signatures: Some(num_signatures),
        pages_per_signature: Some(vec![pages_per_sig; num_signatures]),
        output_pages,
        blank_pages_added,
    })
}

/// Calculate statistics for simple 2-up binding
fn calculate_simple_stats(source_pages: usize) -> Result<ImpositionStatistics> {
    // Perfect binding, side stitch, spiral: 2 pages per sheet
    let padded_count = round_up_to_multiple(source_pages, 2);
    let blank_pages_added = padded_count - source_pages;

    let total_sheets = padded_count / 2;
    let output_pages = total_sheets * 2;

    Ok(ImpositionStatistics {
        source_pages,
        output_sheets: total_sheets,
        signatures: None,
        pages_per_signature: None,
        output_pages,
        blank_pages_added,
    })
}

/// Round up to the nearest multiple
fn round_up_to_multiple(value: usize, multiple: usize) -> usize {
    ((value + multiple - 1) / multiple) * multiple
}
