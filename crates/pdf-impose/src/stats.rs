use crate::options::ImpositionOptions;
use crate::types::*;
use lopdf::Document;

/// Calculate statistics for the imposition
pub fn calculate_statistics(
    documents: &[Document],
    options: &ImpositionOptions,
) -> Result<ImpositionStatistics> {
    // Count total source pages
    let mut source_pages = 0;
    for doc in documents {
        source_pages += doc.get_pages().len();
    }

    // Add flyleaves
    source_pages += options.front_flyleaves + options.back_flyleaves;

    if source_pages == 0 {
        return Err(ImposeError::NoPages);
    }

    // Calculate based on binding type
    match options.binding_type {
        BindingType::Signature | BindingType::CaseBinding => {
            calculate_signature_stats(source_pages, options)
        }
        BindingType::PerfectBinding | BindingType::SideStitch | BindingType::Spiral => {
            calculate_simple_stats(source_pages, options)
        }
    }
}

fn calculate_signature_stats(
    source_pages: usize,
    options: &ImpositionOptions,
) -> Result<ImpositionStatistics> {
    let pages_per_sig = options.page_arrangement.pages_per_signature();

    // Pad to multiple of pages_per_signature
    let padded_count = ((source_pages + pages_per_sig - 1) / pages_per_sig) * pages_per_sig;
    let blank_pages_added = padded_count - source_pages;

    let num_signatures = padded_count / pages_per_sig;
    let sheets_per_sig = pages_per_sig / 4;
    let total_sheets = num_signatures * sheets_per_sig;

    // Calculate pages per signature (for display)
    let mut pages_per_signature = Vec::new();
    for _ in 0..num_signatures {
        pages_per_signature.push(pages_per_sig);
    }

    // Output pages (front and back of each sheet)
    let output_pages = total_sheets * 2;

    Ok(ImpositionStatistics {
        source_pages,
        output_sheets: total_sheets,
        signatures: Some(num_signatures),
        pages_per_signature: Some(pages_per_signature),
        output_pages,
        blank_pages_added,
    })
}

fn calculate_simple_stats(
    source_pages: usize,
    _options: &ImpositionOptions,
) -> Result<ImpositionStatistics> {
    // Perfect binding, side stitch, spiral: 2 pages per sheet
    let padded_count = if source_pages % 2 == 0 {
        source_pages
    } else {
        source_pages + 1
    };

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
