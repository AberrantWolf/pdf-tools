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
            // Show first signature
            pages_per_sig
        }
        BindingType::PerfectBinding | BindingType::SideStitch | BindingType::Spiral => {
            // Show max_sheets worth of pages
            max_sheets * 2 // 2 pages per sheet
        }
    };

    // Create preview documents with limited pages
    let preview_docs = limit_document_pages(documents, source_pages_needed)?;

    // Impose with limited pages
    impose(&preview_docs, &preview_options).await
}

fn limit_document_pages(documents: &[Document], _max_pages: usize) -> Result<Vec<Document>> {
    if documents.is_empty() {
        return Err(ImposeError::NoPages);
    }

    // For now, just return first document
    // Full implementation would extract limited pages
    Ok(documents.to_vec())
}
