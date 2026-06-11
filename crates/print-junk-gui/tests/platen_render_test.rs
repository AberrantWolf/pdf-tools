//! Consumer-side smoke test: proves print-junk's `pdf-viewer` feature wiring
//! renders a PDF through the shared `junk-libs-platen` crate. The render core
//! itself is unit-tested in junk-libs; this guards the consumer dependency and
//! feature plumbing. (Under PDFium this also had to guard a vendored binary
//! binding at runtime — pure-Rust `platen` has nothing to bind.)
#![cfg(all(not(target_arch = "wasm32"), feature = "pdf-viewer"))]

/// Minimal valid one-page PDF (612×792), enough to load and render.
const SAMPLE_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<<
/Type /Catalog
/Pages 2 0 R
>>
endobj
2 0 obj
<<
/Type /Pages
/Kids [3 0 R]
/Count 1
>>
endobj
3 0 obj
<<
/Type /Page
/Parent 2 0 R
/Resources <<
/Font <<
/F1 <<
/Type /Font
/Subtype /Type1
/BaseFont /Helvetica
>>
>>
>>
/MediaBox [0 0 612 792]
/Contents 4 0 R
>>
endobj
4 0 obj
<<
/Length 44
>>
stream
BT
/F1 24 Tf
100 700 Td
(Hello World) Tj
ET
endstream
endobj
xref
0 5
0000000000 65535 f
0000000009 00000 n
0000000058 00000 n
0000000115 00000 n
0000000317 00000 n
trailer
<<
/Size 5
/Root 1 0 R
>>
startxref
410
%%EOF
";

#[test]
fn renders_through_junk_libs_platen() {
    let (image, (width_pts, height_pts)) =
        junk_libs_platen::render_page_bitmap_from_bytes(SAMPLE_PDF, 0, 1.0)
            .expect("render the sample PDF");

    // MediaBox is 612×792 pt; at scale 1.0 the raster matches in pixels.
    assert!(
        (width_pts - 612.0).abs() < 2.0 && (height_pts - 792.0).abs() < 2.0,
        "unexpected page size in points: {width_pts}×{height_pts}"
    );
    assert!(
        image.width() > 0 && image.height() > 0,
        "degenerate raster {}×{}",
        image.width(),
        image.height()
    );
    // The page has text on a white background, so the raster must contain
    // some non-white pixels (the glyphs).
    let non_white = image.pixels().filter(|p| p.0[0] < 200).count();
    assert!(non_white > 0, "rendered image was entirely blank");
}
