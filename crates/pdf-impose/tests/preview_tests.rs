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
async fn test_generate_preview_basic() {
    let doc = create_test_pdf(20);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;

    let preview = generate_preview(&[doc], &options, 2).await;
    assert!(preview.is_ok());

    let output = preview.unwrap();
    // 20 pages with Quarto (8 per signature) padded to 24 / 4 pages per sheet = 6 sheets * 2 sides = 12 pages
    assert_eq!(output.get_pages().len(), 12);
}

#[tokio::test]
async fn test_generate_preview_no_pages() {
    let doc = create_test_pdf(0);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    let preview = generate_preview(&[doc], &options, 1).await;
    assert!(preview.is_err());
}

#[tokio::test]
async fn test_generate_preview_different_sheet_counts() {
    let doc = create_test_pdf(16);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    for max_sheets in 1..=5 {
        let preview = generate_preview(&[doc.clone()], &options, max_sheets).await;
        assert!(preview.is_ok(), "Failed with max_sheets: {}", max_sheets);

        let output = preview.unwrap();
        // 16 pages with default Quarto (8 per signature) / 4 pages per sheet = 4 sheets * 2 sides = 8 pages
        assert_eq!(output.get_pages().len(), 8);
    }
}

#[tokio::test]
async fn test_generate_preview_perfect_binding() {
    let doc = create_test_pdf(12);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::PerfectBinding;

    let preview = generate_preview(&[doc], &options, 3).await;
    assert!(preview.is_ok());

    let output = preview.unwrap();
    // 12 pages with PerfectBinding / 2 pages per sheet = 6 sheets * 2 sides = 12 pages (no padding needed)
    assert_eq!(output.get_pages().len(), 6);
}

#[tokio::test]
async fn test_generate_preview_octavo() {
    let doc = create_test_pdf(32);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Octavo;

    let preview = generate_preview(&[doc], &options, 2).await;
    assert!(preview.is_ok());

    let output = preview.unwrap();
    // 32 pages with Octavo (16 per signature) / 4 pages per sheet = 8 sheets * 2 sides = 16 pages
    assert_eq!(output.get_pages().len(), 16);
}
