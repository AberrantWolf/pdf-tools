//! Test example that generates imposed PDFs for all three main formats:
//! - Folio (4 pages)
//! - Quarto (8 pages)
//! - Octavo (16 pages)
//!
//! Usage: cargo run --example test_all_formats -p pdf-impose
//!
//! This will create test PDFs in the current directory that you can print
//! and fold to verify the imposition is correct.
//!
//! All outputs are sized for A4 paper.
//!
//! ## How to verify:
//!
//! ### Folio (4 pages):
//! 1. Print test_folio_imposed.pdf double-sided (flip on short edge)
//! 2. Fold once along the vertical center
//! 3. Pages should read 1, 2, 3, 4 in order
//!
//! ### Quarto (8 pages):
//! 1. Print test_quarto_imposed.pdf double-sided (flip on short edge)
//! 2. Fold once along the vertical center
//! 3. Fold again along the horizontal center
//! 4. Cut the top fold
//! 5. Pages should read 1, 2, 3, 4, 5, 6, 7, 8 in order
//!
//! ### Octavo (16 pages):
//! 1. Print test_octavo_imposed.pdf double-sided (flip on short edge)
//! 2. Three folds are needed - see detailed instructions in output
//! 3. Pages should read 1-16 in order

use lopdf::{Dictionary, Document, Object, Stream};
use pdf_impose::*;

/// Creates a PDF with large centered page numbers for easy visual verification
/// Page size is A6 (105mm x 148mm) which fits well when imposed onto A4
fn create_numbered_pdf(num_pages: usize) -> Document {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();

    // A6 size in points: 105mm x 148mm = 297.64 x 419.53 points
    let page_width = 298;
    let page_height = 420;

    for page_num in 1..=num_pages {
        // Create content that draws a large centered page number
        // Also draws a border and "TOP" indicator for orientation
        let content = format!(
            r#"
            q
            % Draw border
            1 w
            5 5 {} {} re S
            % Draw "TOP" at top of page
            BT /F1 18 Tf {} {} Td (TOP) Tj ET
            % Draw large page number in center
            BT /F1 120 Tf {} {} Td ({}) Tj ET
            % Draw small page number at bottom
            BT /F1 10 Tf {} 15 Td (Page {}) Tj ET
            Q
            "#,
            page_width - 10,
            page_height - 10,
            (page_width / 2) - 20,  // TOP x position
            page_height - 30,       // TOP y position
            (page_width / 2) - 40,  // Number x position
            (page_height / 2) - 40, // Number y position
            page_num,
            (page_width / 2) - 20, // "Page N" x position
            page_num
        );
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

        // Create font resources
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica-Bold".to_vec()));
        let font_id = doc.add_object(font_dict);

        let mut font_resources = Dictionary::new();
        font_resources.set("F1", Object::Reference(font_id));

        let mut resources = Dictionary::new();
        resources.set("Font", Object::Dictionary(font_resources));

        // Create page (A6 size)
        let page_id = doc.add_object(Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(page_width),
                    Object::Integer(page_height),
                ]),
            ),
            ("Resources", Object::Dictionary(resources)),
            ("Contents", Object::Reference(content_id)),
        ]));
        kids.push(Object::Reference(page_id));
    }

    // Create pages dict
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(kids)),
        ("Count", Object::Integer(num_pages as i64)),
    ]);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Create catalog
    let catalog_id = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));

    doc.trailer.set("Root", catalog_id);
    doc
}

async fn create_test_output(
    num_pages: usize,
    arrangement: PageArrangement,
    paper_size: PaperSize,
    name: &str,
) -> Result<()> {
    let source_doc = create_numbered_pdf(num_pages);

    // Save source
    let source_name = format!("{}_source.pdf", name);
    let mut source_bytes = Vec::new();
    source_doc.clone().save_to(&mut source_bytes).unwrap();
    tokio::fs::write(&source_name, source_bytes).await?;

    // Create imposition options
    let mut options = ImpositionOptions::default();
    options.input_files.push(source_name.clone().into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = arrangement;
    options.output_paper_size = paper_size;
    options.scaling_mode = ScalingMode::Fit;
    options.margins = Margins {
        sheet: SheetMargins {
            top_mm: 5.0,
            bottom_mm: 5.0,
            left_mm: 5.0,
            right_mm: 5.0,
        },
        leaf: LeafMargins {
            top_mm: 5.0,
            bottom_mm: 5.0,
            fore_edge_mm: 3.0,
            spine_mm: 7.0,
        },
    };
    // Enable printer's marks
    options.marks = PrinterMarks {
        fold_lines: true,
        cut_lines: true,
        crop_marks: true,
        registration_marks: true,
        trim_marks: false,
    };

    // Perform imposition
    let imposed = impose(&[source_doc], &options).await?;

    // Save imposed
    let imposed_name = format!("{}_imposed.pdf", name);
    save_pdf(imposed, &imposed_name).await?;

    println!("Created {} and {}", source_name, imposed_name);
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    println!("=== PDF Imposition Test - All Formats (A4 Paper) ===\n");

    // A4 landscape: 297mm x 210mm (width x height)
    let a4_landscape = PaperSize::Custom {
        width_mm: 297.0,
        height_mm: 210.0,
    };

    // Folio: 4 pages -> A4 landscape (2 A6 pages side by side)
    println!("--- FOLIO (4 pages) ---");
    create_test_output(4, PageArrangement::Folio, a4_landscape, "test_folio").await?;
    println!("Output: A4 landscape");
    println!("Expected page order: Side A [4, 1], Side B [2, 3]");
    println!("Fold: One vertical fold down the center");
    println!();

    // Quarto: 8 pages -> A4 portrait (4 pages in 2x2 grid)
    println!("--- QUARTO (8 pages) ---");
    create_test_output(8, PageArrangement::Quarto, PaperSize::A4, "test_quarto").await?;
    println!("Output: A4 portrait");
    println!("Expected page order (per Wikipedia, Side B mirrored for duplex):");
    println!("  Side A: Top [5, 4] (rotated 180), Bottom [8, 1]");
    println!("  Side B: Top [3, 6] (rotated 180), Bottom [2, 7]");
    println!("Folds: First vertical, then horizontal. Cut top edge after folding.");
    println!();

    // Octavo: 16 pages -> A4 landscape (8 pages in 4x2 grid)
    // With A6 source pages, 4 across x 2 down fits on A4 landscape
    println!("--- OCTAVO (16 pages) ---");
    create_test_output(16, PageArrangement::Octavo, a4_landscape, "test_octavo").await?;
    println!("Output: A4 landscape");
    println!("Expected page order (per Wikipedia, Side B mirrored for duplex):");
    println!("  Side A: Top [5, 12, 9, 8] (rotated 180), Bottom [4, 13, 16, 1]");
    println!("  Side B: Top [7, 10, 11, 6] (rotated 180), Bottom [2, 15, 14, 3]");
    println!("Folds: Three folds needed.");
    println!();

    println!("=== Instructions ===");
    println!("1. Print each *_imposed.pdf file double-sided (flip on short edge)");
    println!("2. Fold according to the instructions above");
    println!("3. After folding, pages should be in order 1, 2, 3, ... and right-side up");
    println!("4. The 'TOP' label should appear at the top of each page");
    println!();
    println!("If pages are upside-down or out of order, the rotation/layout logic needs fixing.");

    Ok(())
}
