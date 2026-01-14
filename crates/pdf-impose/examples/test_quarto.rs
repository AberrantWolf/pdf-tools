use lopdf::{Dictionary, Document, Object, Stream};
use pdf_impose::*;

fn create_numbered_pdf(num_pages: usize) -> Document {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();

    for page_num in 1..=num_pages {
        // Create a content stream that draws the page number
        let content = format!("BT /F1 200 Tf 200 350 Td ({}) Tj ET", page_num);
        let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));

        // Create font resources
        let mut font_dict = Dictionary::new();
        font_dict.set("Type", Object::Name(b"Font".to_vec()));
        font_dict.set("Subtype", Object::Name(b"Type1".to_vec()));
        font_dict.set("BaseFont", Object::Name(b"Helvetica".to_vec()));
        let font_id = doc.add_object(font_dict);

        let mut font_resources = Dictionary::new();
        font_resources.set("F1", Object::Reference(font_id));

        let mut resources = Dictionary::new();
        resources.set("Font", Object::Dictionary(font_resources));

        // Create page
        let page_id = doc.add_object(Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(612),
                    Object::Integer(792),
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    // Create a test PDF with 8 pages (perfect for quarto)
    let source_doc = create_numbered_pdf(8);

    // Save the source for reference
    let mut source_bytes = Vec::new();
    source_doc.clone().save_to(&mut source_bytes).unwrap();
    tokio::fs::write("test_source.pdf", source_bytes).await?;
    println!("Created test_source.pdf with 8 numbered pages");

    // Create imposition options for quarto
    let mut options = ImpositionOptions::default();
    options.input_files.push("test_source.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.output_paper_size = PaperSize::Tabloid; // 11x17 for quarto
    options.scaling_mode = ScalingMode::Fit;

    // Set margins to make them visible
    options.margins = Margins {
        top_mm: 15.0,
        bottom_mm: 15.0,
        fore_edge_mm: 10.0,
        spine_mm: 20.0, // Larger spine margin to see the effect
    };

    // Perform imposition
    let imposed = impose(&[source_doc], &options).await?;

    // Save the imposed PDF
    save_pdf(imposed, "test_quarto_imposed.pdf").await?;
    println!("Created test_quarto_imposed.pdf");
    println!("\nExpected layout:");
    println!("  Front side (sheet 1): pages 8, 1, 2, 7 (pages 8,1 rotated 180°)");
    println!("  Back side (sheet 1):  pages 6, 3, 4, 5 (pages 4,5 rotated 180°)");
    println!("\nMargins:");
    println!("  Spine (inner): {}mm", options.margins.spine_mm);
    println!("  Fore-edge (outer): {}mm", options.margins.fore_edge_mm);
    println!("\nPages should be pushed toward the spine (center), not centered.");

    Ok(())
}
