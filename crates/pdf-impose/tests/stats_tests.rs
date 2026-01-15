use lopdf::{Dictionary, Document, Object, Stream};
use pdf_impose::*;

fn create_test_document(num_pages: usize) -> Document {
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

#[test]
fn test_stats_no_pages() {
    let doc = create_test_document(0);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());

    let result = calculate_statistics(&[doc], &options);
    assert!(result.is_err());
    match result {
        Err(ImposeError::NoPages) => {}
        _ => panic!("Expected NoPages error"),
    }
}

#[test]
fn test_stats_quarto_signature() {
    let doc = create_test_document(10);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto; // 8 pages per signature

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 10);
    // 10 pages padded to 16 (2 signatures of 8 pages each)
    assert_eq!(stats.blank_pages_added, 6);
    assert_eq!(stats.signatures, Some(2));
    // 16 pages / 4 pages per sheet = 4 sheets
    assert_eq!(stats.output_sheets, 4);
    // 4 sheets * 2 sides = 8 output pages
    assert_eq!(stats.output_pages, 8);
}

#[test]
fn test_stats_folio_signature() {
    let doc = create_test_document(6);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Folio; // 4 pages per signature

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 6);
    // 6 pages padded to 8 (2 signatures of 4 pages each)
    assert_eq!(stats.blank_pages_added, 2);
    assert_eq!(stats.signatures, Some(2));
    // 8 pages / 4 pages per sheet = 2 sheets
    assert_eq!(stats.output_sheets, 2);
    // 2 sheets * 2 sides = 4 output pages
    assert_eq!(stats.output_pages, 4);
}

#[test]
fn test_stats_octavo_signature() {
    let doc = create_test_document(20);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Octavo; // 16 pages per signature

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 20);
    // 20 pages padded to 32 (2 signatures of 16 pages each)
    assert_eq!(stats.blank_pages_added, 12);
    assert_eq!(stats.signatures, Some(2));
    // 32 pages / 4 pages per sheet = 8 sheets
    assert_eq!(stats.output_sheets, 8);
    // 8 sheets * 2 sides = 16 output pages
    assert_eq!(stats.output_pages, 16);
}

#[test]
fn test_stats_custom_signature() {
    let doc = create_test_document(15);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Custom {
        pages_per_signature: 12,
    };

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 15);
    // 15 pages padded to 24 (2 signatures of 12 pages each)
    assert_eq!(stats.blank_pages_added, 9);
    assert_eq!(stats.signatures, Some(2));
    // 24 pages / 4 pages per sheet = 6 sheets
    assert_eq!(stats.output_sheets, 6);
    // 6 sheets * 2 sides = 12 output pages
    assert_eq!(stats.output_pages, 12);
}

#[test]
fn test_stats_perfect_binding() {
    let doc = create_test_document(11);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::PerfectBinding;

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 11);
    // 11 pages padded to 12 (even number for 2-up)
    assert_eq!(stats.blank_pages_added, 1);
    assert_eq!(stats.signatures, None);
    // 12 pages / 2 pages per sheet = 6 sheets
    assert_eq!(stats.output_sheets, 6);
    // 6 sheets * 2 sides = 12 output pages
    assert_eq!(stats.output_pages, 12);
}

#[test]
fn test_stats_with_flyleaves() {
    let doc = create_test_document(10);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto; // 8 pages per signature
    options.front_flyleaves = 2;
    options.back_flyleaves = 2;

    let stats = calculate_statistics(&[doc], &options).unwrap();

    // 10 original + (2 front flyleaves * 2 pages) + (2 back flyleaves * 2 pages) = 18 pages
    // Each flyleaf is 1 leaf = 2 pages (front and back)
    assert_eq!(stats.source_pages, 18);
    // 18 pages padded to 24 (3 signatures of 8 pages each)
    assert_eq!(stats.blank_pages_added, 6);
    assert_eq!(stats.signatures, Some(3));
    assert_eq!(stats.output_sheets, 6); // 3 signatures * 2 sheets per signature
    assert_eq!(stats.output_pages, 12); // 6 sheets * 2 sides
}

#[test]
fn test_stats_exact_signature_fit() {
    let doc = create_test_document(16);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Octavo; // 16 pages per signature

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 16);
    // Perfect fit, no padding needed
    assert_eq!(stats.blank_pages_added, 0);
    assert_eq!(stats.signatures, Some(1));
    assert_eq!(stats.output_sheets, 4);
    assert_eq!(stats.output_pages, 8);
}

#[test]
fn test_stats_side_stitch() {
    let doc = create_test_document(7);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::SideStitch;

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 7);
    assert_eq!(stats.blank_pages_added, 1); // Padded to 8
    assert_eq!(stats.signatures, None);
    assert_eq!(stats.output_sheets, 4);
    assert_eq!(stats.output_pages, 8);
}

#[test]
fn test_stats_spiral() {
    let doc = create_test_document(5);
    let mut options = ImpositionOptions::default();
    options.input_files.push("test.pdf".into());
    options.binding_type = BindingType::Spiral;

    let stats = calculate_statistics(&[doc], &options).unwrap();

    assert_eq!(stats.source_pages, 5);
    assert_eq!(stats.blank_pages_added, 1); // Padded to 6
    assert_eq!(stats.signatures, None);
    assert_eq!(stats.output_sheets, 3);
    assert_eq!(stats.output_pages, 6);
}
