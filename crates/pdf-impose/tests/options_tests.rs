use pdf_impose::*;
use std::path::PathBuf;

#[test]
fn test_validation_no_input_files() {
    let options = ImpositionOptions::default();
    let result = options.validate();
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(msg)) => {
            assert!(msg.contains("No input files"));
        }
        _ => panic!("Expected Config error"),
    }
}

#[test]
fn test_validation_invalid_pages_per_signature() {
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    // Valid: 4 pages per signature
    options.page_arrangement = PageArrangement::Folio;
    assert!(options.validate().is_ok());

    // Valid: 8 pages per signature
    options.page_arrangement = PageArrangement::Quarto;
    assert!(options.validate().is_ok());

    // Valid: 16 pages per signature
    options.page_arrangement = PageArrangement::Octavo;
    assert!(options.validate().is_ok());

    // Invalid: 0 pages
    options.page_arrangement = PageArrangement::Custom {
        pages_per_signature: 0,
    };
    assert!(options.validate().is_err());

    // Invalid: not multiple of 4
    options.page_arrangement = PageArrangement::Custom {
        pages_per_signature: 6,
    };
    assert!(options.validate().is_err());

    // Invalid: not multiple of 4
    options.page_arrangement = PageArrangement::Custom {
        pages_per_signature: 3,
    };
    assert!(options.validate().is_err());

    // Valid: 12 pages (multiple of 4)
    options.page_arrangement = PageArrangement::Custom {
        pages_per_signature: 12,
    };
    assert!(options.validate().is_ok());
}

#[cfg(feature = "serde")]
#[tokio::test]
async fn test_save_and_load_options() {
    use tempfile::NamedTempFile;

    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("input.pdf"));
    options.binding_type = BindingType::PerfectBinding;
    options.page_arrangement = PageArrangement::Octavo;
    options.output_paper_size = PaperSize::A4;
    options.front_flyleaves = 2;
    options.back_flyleaves = 1;
    options.add_page_numbers = true;

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Save
    options.save(path).await.unwrap();

    // Load
    let loaded = ImpositionOptions::load(path).await.unwrap();

    assert_eq!(loaded.input_files, options.input_files);
    assert_eq!(loaded.binding_type, options.binding_type);
    assert_eq!(loaded.page_arrangement, options.page_arrangement);
    assert_eq!(loaded.output_paper_size, options.output_paper_size);
    assert_eq!(loaded.front_flyleaves, options.front_flyleaves);
    assert_eq!(loaded.back_flyleaves, options.back_flyleaves);
    assert_eq!(loaded.add_page_numbers, options.add_page_numbers);
}
