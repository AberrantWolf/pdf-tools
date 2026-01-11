#[cfg(all(test, not(target_arch = "wasm32"), feature = "pdf-viewer"))]
mod pdfium_render_tests {
    use pdfium_render::prelude::*;

    /// Minimal valid PDF document (Hello World)
    /// This is a complete, valid PDF that contains simple text
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
    fn test_pdfium_loads_and_renders() {
        // This test verifies that pdfium is correctly installed, linked, and functioning
        // by loading a PDF and rendering it to an image

        // Initialize Pdfium library - explicitly load from the vendor directory
        // to avoid conflicts with system-installed or user-local pdfium libraries
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .expect("Failed to find workspace root");
        let pdfium_lib_path = workspace_root.join("vendor/pdfium/lib");
        let pdfium_lib_name = if cfg!(target_os = "macos") {
            "libpdfium.dylib"
        } else if cfg!(target_os = "windows") {
            "pdfium.dll"
        } else {
            "libpdfium.so"
        };

        let pdfium = Pdfium::new(
            Pdfium::bind_to_library(
                pdfium_lib_path
                    .join(pdfium_lib_name)
                    .to_str()
                    .expect("Invalid pdfium library path"),
            )
            .expect(
                "Failed to bind to Pdfium library. Make sure pdfium is installed via build script.",
            ),
        );

        // Load the sample PDF from memory
        let document = pdfium
            .load_pdf_from_byte_slice(SAMPLE_PDF, None)
            .expect("Failed to load PDF document");

        // Verify the document has at least one page
        assert_eq!(
            document.pages().len(),
            1,
            "Sample PDF should have exactly 1 page"
        );

        // Get the first page
        let page = document.pages().get(0).expect("Failed to get first page");

        // Verify page dimensions are reasonable
        let width = page.width().value;
        let height = page.height().value;
        assert!(width > 0.0, "Page width should be positive");
        assert!(height > 0.0, "Page height should be positive");

        // Render the page to a bitmap at 72 DPI
        let render_config = PdfRenderConfig::new()
            .set_target_width(612)
            .set_maximum_height(792);

        let bitmap = page
            .render_with_config(&render_config)
            .expect("Failed to render page to bitmap");

        // Verify the bitmap was created with expected dimensions
        let bitmap_width = bitmap.width();
        let bitmap_height = bitmap.height();

        assert!(bitmap_width > 0, "Bitmap width should be positive");
        assert!(bitmap_height > 0, "Bitmap height should be positive");

        // Verify we can convert to image format (this exercises the full rendering pipeline)
        let image_buffer = bitmap.as_image().into_rgb8();

        assert_eq!(
            image_buffer.width(),
            bitmap_width as u32,
            "Image buffer width should match bitmap width"
        );
        assert_eq!(
            image_buffer.height(),
            bitmap_height as u32,
            "Image buffer height should match bitmap height"
        );

        // Verify the image has actual data (not all zeros)
        let pixels: Vec<u8> = image_buffer.into_raw();
        let non_zero_pixels = pixels.iter().filter(|&&p| p != 0).count();

        assert!(
            non_zero_pixels > 0,
            "Rendered image should contain some non-zero pixels (actual content)"
        );

        println!(
            "✓ Pdfium successfully loaded and rendered PDF ({}x{} bitmap, {} non-zero pixels)",
            bitmap_width, bitmap_height, non_zero_pixels
        );
    }
}
