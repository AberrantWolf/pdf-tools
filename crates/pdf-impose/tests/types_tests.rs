use pdf_impose::*;

#[test]
fn test_paper_size_dimensions() {
    let a4 = PaperSize::A4;
    assert_eq!(a4.dimensions_mm(), (210.0, 297.0));

    let a3 = PaperSize::A3;
    assert_eq!(a3.dimensions_mm(), (297.0, 420.0));

    let a5 = PaperSize::A5;
    assert_eq!(a5.dimensions_mm(), (148.0, 210.0));

    let letter = PaperSize::Letter;
    assert_eq!(letter.dimensions_mm(), (215.9, 279.4));

    let legal = PaperSize::Legal;
    assert_eq!(legal.dimensions_mm(), (215.9, 355.6));

    let tabloid = PaperSize::Tabloid;
    assert_eq!(tabloid.dimensions_mm(), (279.4, 431.8));

    let custom = PaperSize::Custom {
        width_mm: 100.0,
        height_mm: 200.0,
    };
    assert_eq!(custom.dimensions_mm(), (100.0, 200.0));
}

#[test]
fn test_page_arrangement_pages_per_signature() {
    assert_eq!(PageArrangement::Folio.pages_per_signature(), 4);
    assert_eq!(PageArrangement::Quarto.pages_per_signature(), 8);
    assert_eq!(PageArrangement::Octavo.pages_per_signature(), 16);
    assert_eq!(
        PageArrangement::Custom {
            pages_per_signature: 12
        }
        .pages_per_signature(),
        12
    );
}

#[test]
fn test_rotation_degrees() {
    assert_eq!(Rotation::None.degrees(), 0);
    assert_eq!(Rotation::Clockwise90.degrees(), 90);
    assert_eq!(Rotation::Clockwise180.degrees(), 180);
    assert_eq!(Rotation::Clockwise270.degrees(), 270);
}

#[test]
fn test_margins_default() {
    let margins = Margins::default();
    // Sheet margins (printer-safe area)
    assert_eq!(margins.sheet.top_mm, 5.0);
    assert_eq!(margins.sheet.bottom_mm, 5.0);
    assert_eq!(margins.sheet.left_mm, 5.0);
    assert_eq!(margins.sheet.right_mm, 5.0);
    // Leaf margins (trim and gutter)
    assert_eq!(margins.leaf.top_mm, 5.0);
    assert_eq!(margins.leaf.bottom_mm, 5.0);
    assert_eq!(margins.leaf.fore_edge_mm, 5.0);
    assert_eq!(margins.leaf.spine_mm, 10.0);
}

#[test]
fn test_printer_marks_default() {
    let marks = PrinterMarks::default();
    assert!(!marks.fold_lines);
    assert!(!marks.crop_marks);
    assert!(!marks.registration_marks);
    assert!(!marks.sewing_marks);
    assert!(!marks.spine_marks);
}
