use lopdf::{Dictionary, Document, Object, Stream};
use pdf_impose::*;
use std::path::PathBuf;

fn create_test_pdf(num_pages: usize) -> Document {
    let mut doc = Document::with_version("1.7");

    // Create page tree root ID
    let pages_id = doc.new_object_id();

    // Create pages array
    let mut kids = Vec::new();
    for _ in 0..num_pages {
        let content_id = doc.add_object(Stream::new(Dictionary::new(), b"q Q".to_vec()));

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
            ("Resources", Object::Dictionary(Dictionary::new())),
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

#[tokio::test]
async fn test_load_pdf() {
    use tempfile::NamedTempFile;

    let mut doc = create_test_pdf(5);
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    // Save test PDF
    let mut writer = Vec::new();
    doc.save_to(&mut writer).unwrap();
    std::fs::write(path, writer).unwrap();

    // Load it back
    let loaded = load_pdf(path).await.unwrap();
    assert_eq!(loaded.get_pages().len(), 5);
}

#[tokio::test]
async fn test_load_multiple_pdfs() {
    use tempfile::NamedTempFile;

    let mut doc1 = create_test_pdf(3);
    let mut doc2 = create_test_pdf(4);

    let temp1 = NamedTempFile::new().unwrap();
    let temp2 = NamedTempFile::new().unwrap();

    let mut writer = Vec::new();
    doc1.save_to(&mut writer).unwrap();
    std::fs::write(temp1.path(), &writer).unwrap();

    writer.clear();
    doc2.save_to(&mut writer).unwrap();
    std::fs::write(temp2.path(), &writer).unwrap();

    let paths = vec![temp1.path(), temp2.path()];
    let docs = load_multiple_pdfs(&paths).await.unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].get_pages().len(), 3);
    assert_eq!(docs[1].get_pages().len(), 4);
}

#[tokio::test]
async fn test_save_pdf() {
    use tempfile::NamedTempFile;

    let doc = create_test_pdf(2);
    let temp = NamedTempFile::new().unwrap();

    save_pdf(doc, temp.path()).await.unwrap();

    // Verify file was created and can be loaded
    assert!(temp.path().exists());
    let loaded = Document::load(temp.path()).unwrap();
    assert_eq!(loaded.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_no_pages() {
    let doc = create_test_pdf(0);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    let result = impose(&[doc], &options).await;
    assert!(result.is_err());
    match result {
        Err(ImposeError::NoPages) => {}
        _ => panic!("Expected NoPages error"),
    }
}

#[tokio::test]
async fn test_impose_validation_fails() {
    let doc = create_test_pdf(5);
    let options = ImpositionOptions::default(); // No input files

    let result = impose(&[doc], &options).await;
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(_)) => {}
        _ => panic!("Expected Config error"),
    }
}

#[tokio::test]
async fn test_impose_signature_basic() {
    let doc = create_test_pdf(8);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Quarto: 8 pages per signature = 1 signature = 1 sheet with 4 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_perfect_binding() {
    let doc = create_test_pdf(10);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::PerfectBinding;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // PerfectBinding with 10 pages results in 5 output pages
    assert_eq!(output.get_pages().len(), 5);
}

#[tokio::test]
async fn test_impose_with_different_paper_sizes() {
    let doc = create_test_pdf(4);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    // Test different paper sizes
    let paper_sizes = vec![
        PaperSize::A3,
        PaperSize::A4,
        PaperSize::A5,
        PaperSize::Letter,
        PaperSize::Legal,
        PaperSize::Tabloid,
        PaperSize::Custom {
            width_mm: 200.0,
            height_mm: 300.0,
        },
    ];

    for paper_size in paper_sizes {
        options.output_paper_size = paper_size;
        let result = impose(&[doc.clone()], &options).await;
        assert!(result.is_ok(), "Failed for paper size: {:?}", paper_size);
    }
}

#[tokio::test]
async fn test_impose_with_scaling_modes() {
    let doc = create_test_pdf(4);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    let scaling_modes = vec![
        ScalingMode::Fit,
        ScalingMode::Fill,
        ScalingMode::None,
        ScalingMode::Stretch,
    ];

    for mode in scaling_modes {
        options.scaling_mode = mode;
        let result = impose(&[doc.clone()], &options).await;
        assert!(result.is_ok(), "Failed for scaling mode: {:?}", mode);
    }
}

#[tokio::test]
async fn test_impose_folio() {
    let doc = create_test_pdf(4);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Folio;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Folio: 4 pages per signature = 1 signature = 1 sheet with 2 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_octavo() {
    let doc = create_test_pdf(16);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Octavo;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Octavo: 16 pages per signature = 1 signature = 1 sheet with 8 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_with_custom_arrangement() {
    let doc = create_test_pdf(12);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Custom {
        pages_per_signature: 12,
    };

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Custom: 12 pages per signature = 1 signature = 1 sheet with 6 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_side_stitch() {
    let doc = create_test_pdf(6);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::SideStitch;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // SideStitch: simple 2-up layout, 6 pages = 3 sheets × 2 sides = 3 output pages (alternating front/back)
    assert_eq!(output.get_pages().len(), 3);
}

#[tokio::test]
async fn test_impose_spiral() {
    let doc = create_test_pdf(8);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Spiral;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Spiral: simple 2-up layout, 8 pages = 4 sheets × 2 sides = 4 output pages (alternating front/back)
    assert_eq!(output.get_pages().len(), 4);
}

#[tokio::test]
async fn test_impose_case_binding() {
    let doc = create_test_pdf(16);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::CaseBinding;

    let result = impose(&[doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // CaseBinding uses default Quarto: 16 pages = 2 signatures × 8 pages = 2 sheets × 2 sides = 4 output pages
    assert_eq!(output.get_pages().len(), 4);
}

#[tokio::test]
async fn test_full_workflow() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.pdf");
    let output_path = temp_dir.path().join("output.pdf");

    // Create and save input PDF
    let mut doc = create_test_pdf(10);
    let mut writer = Vec::new();
    doc.save_to(&mut writer).unwrap();
    std::fs::write(&input_path, writer).unwrap();

    // Load the PDF
    let loaded = load_pdf(&input_path).await.unwrap();
    assert_eq!(loaded.get_pages().len(), 10);

    // Set up imposition options
    let mut options = ImpositionOptions::default();
    options.input_files.push(input_path.clone());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.output_paper_size = PaperSize::Letter;

    // Perform imposition
    let imposed = impose(&[loaded], &options).await.unwrap();

    // Save output
    save_pdf(imposed, &output_path).await.unwrap();

    // Verify output exists
    assert!(output_path.exists());
}

// Test correct page ordering for traditional bookbinding formats
// These tests verify the actual page sequence matches traditional bookbinding standards
// Note: Page ordering tests are now in the layout::signature module.
// The tests below verify the high-level API behavior.

#[test]
fn test_folio_page_order() {
    // Folio: 1 fold = 4 pages per signature
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::map_pages_to_slots;

    let order = map_pages_to_slots(PageArrangement::Folio, 0, 4);

    // Side A: [4, 1], Side B: [2, 3]
    assert_eq!(order.len(), 4);
    assert_eq!(order[0], Some(3)); // Side A left: page 4
    assert_eq!(order[1], Some(0)); // Side A right: page 1
    assert_eq!(order[2], Some(1)); // Side B left: page 2
    assert_eq!(order[3], Some(2)); // Side B right: page 3
}

#[test]
fn test_quarto_page_order() {
    // Quarto: 2 folds = 8 pages per signature
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::map_pages_to_slots;

    let order = map_pages_to_slots(PageArrangement::Quarto, 0, 8);

    assert_eq!(order.len(), 8);

    // Side A: [5, 4, 8, 1] (top-left, top-right, bottom-left, bottom-right)
    assert_eq!(order[0], Some(4)); // page 5 (0-indexed = 4)
    assert_eq!(order[1], Some(3)); // page 4 (0-indexed = 3)
    assert_eq!(order[2], Some(7)); // page 8 (0-indexed = 7)
    assert_eq!(order[3], Some(0)); // page 1 (0-indexed = 0)

    // Side B: [3, 6, 2, 7] - mirrored for duplex
    assert_eq!(order[4], Some(2)); // page 3 (0-indexed = 2)
    assert_eq!(order[5], Some(5)); // page 6 (0-indexed = 5)
    assert_eq!(order[6], Some(1)); // page 2 (0-indexed = 1)
    assert_eq!(order[7], Some(6)); // page 7 (0-indexed = 6)
}

#[test]
fn test_octavo_page_order() {
    // Octavo: 3 folds = 16 pages per signature
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::map_pages_to_slots;

    let order = map_pages_to_slots(PageArrangement::Octavo, 0, 16);

    assert_eq!(order.len(), 16);

    // Expected sequence (0-indexed):
    // Side A top:    [5, 12, 9, 8]  -> [4, 11, 8, 7]
    // Side A bottom: [4, 13, 16, 1] -> [3, 12, 15, 0]
    // Side B top:    [7, 10, 11, 6] -> [6, 9, 10, 5] (mirrored for duplex)
    // Side B bottom: [2, 15, 14, 3] -> [1, 14, 13, 2] (mirrored for duplex)
    let expected = vec![
        4, 11, 8, 7, // Side A top row
        3, 12, 15, 0, // Side A bottom row
        5, 10, 9, 6, // Side B top row (mirrored)
        2, 13, 14, 1, // Side B bottom row (mirrored)
    ];

    for (i, &expected_page) in expected.iter().enumerate() {
        assert_eq!(
            order[i],
            Some(expected_page),
            "Mismatch at position {}: expected page {}, got {:?}",
            i,
            expected_page + 1,
            order[i].map(|p| p + 1)
        );
    }
}

#[test]
fn test_multiple_signatures() {
    // Test with 2 quarto signatures (16 pages total)
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::map_pages_to_slots;

    // First signature
    let order1 = map_pages_to_slots(PageArrangement::Quarto, 0, 16);
    // Second signature
    let order2 = map_pages_to_slots(PageArrangement::Quarto, 8, 16);

    // First signature: pages 1-8 (indices 0-7)
    // Side A: [5, 4, 8, 1] -> [4, 3, 7, 0]
    assert_eq!(order1[0], Some(4)); // page 5
    assert_eq!(order1[1], Some(3)); // page 4
    assert_eq!(order1[2], Some(7)); // page 8
    assert_eq!(order1[3], Some(0)); // page 1

    // Second signature: pages 9-16 (indices 8-15)
    // Side A: [13, 12, 16, 9] -> [12, 11, 15, 8]
    assert_eq!(order2[0], Some(12)); // page 13
    assert_eq!(order2[1], Some(11)); // page 12
    assert_eq!(order2[2], Some(15)); // page 16
    assert_eq!(order2[3], Some(8)); // page 9
}
