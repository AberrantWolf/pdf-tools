use lopdf::Document;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ImposeError {
    #[error("PDF error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid configuration: {0}")]
    Config(String),
    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, ImposeError>;

#[derive(Debug, Clone, Copy)]
pub enum ImpositionLayout {
    /// 2-up: two pages side by side
    TwoUp,
    /// 4-up: four pages in a 2x2 grid
    FourUp,
    /// Booklet: reorder pages for saddle-stitch binding
    Booklet,
    /// N-up with custom rows and columns
    NUp { rows: u32, cols: u32 },
}

#[derive(Debug, Clone)]
pub struct ImpositionOptions {
    pub layout: ImpositionLayout,
    pub output_width_mm: f32,
    pub output_height_mm: f32,
    pub margin_mm: f32,
}

impl Default for ImpositionOptions {
    fn default() -> Self {
        Self {
            layout: ImpositionLayout::TwoUp,
            output_width_mm: 279.4,  // Letter landscape width
            output_height_mm: 215.9, // Letter landscape height
            margin_mm: 5.0,
        }
    }
}

/// Load a PDF document
pub async fn load_pdf(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref().to_owned();

    // Read file async
    let bytes = tokio::fs::read(&path).await?;

    // Parse is CPU-bound, spawn blocking
    let doc = tokio::task::spawn_blocking(move || Document::load_mem(&bytes)).await??;

    Ok(doc)
}

/// Impose a PDF with the given layout
pub async fn impose(doc: &Document, options: &ImpositionOptions) -> Result<Document> {
    let doc = doc.clone(); // lopdf Document is cheap to clone (Arc internally)
    let options = options.clone();

    // Imposition is CPU-bound
    let imposed = tokio::task::spawn_blocking(move || impose_sync(&doc, &options)).await??;

    Ok(imposed)
}

fn impose_sync(doc: &Document, options: &ImpositionOptions) -> Result<Document> {
    let page_count = doc.get_pages().len();

    match options.layout {
        ImpositionLayout::TwoUp => impose_n_up(doc, 1, 2, options),
        ImpositionLayout::FourUp => impose_n_up(doc, 2, 2, options),
        ImpositionLayout::NUp { rows, cols } => impose_n_up(doc, rows, cols, options),
        ImpositionLayout::Booklet => impose_booklet(doc, page_count, options),
    }
}

fn impose_n_up(
    _doc: &Document,
    rows: u32,
    cols: u32,
    _options: &ImpositionOptions,
) -> Result<Document> {
    let output = Document::with_version("1.7");
    let pages_per_sheet = (rows * cols) as usize;
    let page_ids: Vec<_> = _doc.get_pages().keys().copied().collect();

    // Implementation: scale and position source pages onto output sheets
    // This is a simplified skeleton - full implementation would use
    // lopdf's content stream manipulation

    for _chunk in page_ids.chunks(pages_per_sheet) {
        // Create new page with imposed content
        // ... (full implementation would transform and place each source page)
    }

    Ok(output)
}

fn impose_booklet(
    _doc: &Document,
    page_count: usize,
    _options: &ImpositionOptions,
) -> Result<Document> {
    // Booklet imposition reorders pages for saddle-stitch binding
    // Sheet 1 front: [last, first], Sheet 1 back: [second, second-to-last], etc.

    let output = Document::with_version("1.7");

    // Pad to multiple of 4
    let padded_count = ((page_count + 3) / 4) * 4;

    // Calculate booklet page order
    let mut order = Vec::with_capacity(padded_count);
    for sheet in 0..(padded_count / 4) {
        let base = sheet * 2;
        // Front: outer pages
        order.push(padded_count - 1 - base);
        order.push(base);
        // Back: inner pages
        order.push(base + 1);
        order.push(padded_count - 2 - base);
    }

    // ... (apply 2-up layout with this ordering)

    Ok(output)
}

/// Save the imposed document
pub async fn save_pdf(mut doc: Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref().to_owned();

    // Save to bytes is CPU-bound
    let bytes = tokio::task::spawn_blocking(move || {
        let mut writer = Vec::new();
        doc.save_to(&mut writer)?;
        Ok::<_, ImposeError>(writer)
    })
    .await??;

    // Write async
    tokio::fs::write(&path, bytes).await?;

    Ok(())
}
