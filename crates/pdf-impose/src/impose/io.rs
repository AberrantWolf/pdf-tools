//! Document I/O operations for imposition

use crate::types::*;
use lopdf::Document;
use std::path::Path;

/// Load a single PDF document
pub async fn load_pdf(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref().to_owned();
    let bytes = tokio::fs::read(&path).await?;
    let doc = tokio::task::spawn_blocking(move || Document::load_mem(&bytes)).await??;
    Ok(doc)
}

/// Load multiple PDF documents
pub async fn load_multiple_pdfs(paths: &[impl AsRef<Path>]) -> Result<Vec<Document>> {
    let mut documents = Vec::new();
    for path in paths {
        documents.push(load_pdf(path).await?);
    }
    Ok(documents)
}

/// Save the imposed document
pub async fn save_pdf(mut doc: Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref().to_owned();
    let bytes = tokio::task::spawn_blocking(move || {
        let mut writer = Vec::new();
        doc.save_to(&mut writer)?;
        Ok::<_, ImposeError>(writer)
    })
    .await??;
    tokio::fs::write(&path, bytes).await?;
    Ok(())
}

/// Merge multiple documents into one
pub(crate) fn merge_documents(documents: &[Document]) -> Result<Document> {
    if documents.is_empty() {
        return Err(ImposeError::NoPages);
    }

    if documents.len() == 1 {
        return Ok(documents[0].clone());
    }

    // TODO: Properly merge all pages with resources
    Ok(documents[0].clone())
}
