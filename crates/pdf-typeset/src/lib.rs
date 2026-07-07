//! Lay out a text document (Plaintext / Markdown / HTML) into a typeset PDF.
//!
//! Built on the [Typst](https://typst.app) typesetting engine: input is parsed
//! and emitted as Typst markup ([`markup`]), wrapped in a configurable template
//! ([`template`]) derived from a [`TypesetConfig`], then compiled to PDF bytes.
//!
//! Desktop-only — Typst is a large, native-oriented dependency, so this crate is
//! excluded from the WASM build (gated by the GUI's `typesetting` feature).

mod asset;
mod config;
mod html;
mod markup;
mod math;
mod outline;
mod template;
mod typst_table;

pub use config::{
    BreakPosition, Color, FontChoice, HAlign, HeadingStyle, InputFormat, PageBreakRule,
    TableBorder, TableStyle, TypesetConfig, TypesetInput,
};
pub use html::{AssetResolver, CapturingResolver, ImportStats, ImportedDoc, MapResolver, NoAssets};
pub use markup::import_markdown;
pub use math::{MathAsset, MathPipeline, MathRender, MathSource, Tex2TypstRs, TexMathEngine, Tier};
pub use outline::{FRONT_MATTER_ID, OutlineEntry, SectionOverride, assemble_body};

use typst::layout::PagedDocument;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_as_lib::{TypstEngine, TypstTemplateMainFile};

/// Errors produced while typesetting.
#[derive(Debug, thiserror::Error)]
pub enum TypesetError {
    #[error("Typst compilation failed: {0}")]
    Compile(String),
    #[error("PDF export failed: {0}")]
    Pdf(String),
}

/// Preferred body fonts, in order, used when no font is chosen. The first one
/// installed on the system wins; if none are present, Typst's own default is used.
const DEFAULT_SERIF_PREFERENCES: &[&str] = &[
    "Libertinus Serif",
    "EB Garamond",
    "Georgia",
    "Times New Roman",
    "Liberation Serif",
    "Source Serif 4",
    "Noto Serif",
    "DejaVu Serif",
];

/// Typeset `input` into PDF bytes according to `config`.
pub fn typeset(input: &TypesetInput, config: &TypesetConfig) -> Result<Vec<u8>, TypesetError> {
    let effective = effective_config(config);
    let body = markup::to_typst_body(input, &effective.page_breaks, effective.smart_punctuation);
    let source = template::build_source(&effective, &body);
    let engine = build_engine(source, TypstKitFontOptions::default(), &[]);
    engine_to_pdf(&engine)
}

/// Convert a structured HTML document (e.g. arXiv/LaTeXML) into a typesettable
/// [`ImportedDoc`] — Typst `body` markup plus its assets (math SVGs, fetched
/// images). This is the expensive, config-independent half of an import: it
/// walks the DOM and validates every equation. Images are fetched through
/// `resolver`; the source's table of contents and page chrome are dropped.
///
/// The body is content only. The extracted [`ImportedDoc::title`] is returned for
/// the caller to feed into [`TypesetConfig::doc_title`] — the template owns all
/// front matter (title page, table of contents), so the import never duplicates it.
///
/// Separated from [`compile_imported`] so callers can convert once and re-compile
/// to different page sizes/margins cheaply. The logged [`ImportStats`] record how
/// the math degraded across tiers.
pub fn import_html(html: &str, resolver: &dyn AssetResolver) -> ImportedDoc {
    let doc = html::import(html, resolver);
    let s = &doc.stats;
    log::info!(
        "imported: math {} native / {} image / {} raw, images {} ok / {} failed, {} footnotes, {} citations",
        s.math_tex,
        s.math_image,
        s.math_raw,
        s.images_ok,
        s.images_failed,
        s.footnotes,
        s.citations
    );
    doc
}

/// Compile an already-converted [`ImportedDoc`] to PDF bytes under `config`. The
/// cheap, repeatable half of an import: no DOM walk, no network — only the
/// template (page size, margins, fonts) varies.
pub fn compile_imported(
    doc: &ImportedDoc,
    config: &TypesetConfig,
) -> Result<Vec<u8>, TypesetError> {
    compile_body(&doc.body, &doc.assets, config)
}

/// Compile pre-converted Typst `body` markup plus its named `assets` to PDF under
/// `config`. The primitive behind [`compile_imported`]; taking slices lets the GUI
/// recompile a cached (e.g. `Arc`-shared) import without cloning its asset bytes.
pub fn compile_body(
    body: &str,
    assets: &[(String, Vec<u8>)],
    config: &TypesetConfig,
) -> Result<Vec<u8>, TypesetError> {
    let effective = effective_config(config);
    let source = template::build_source(&effective, body);
    let engine = build_engine(source, TypstKitFontOptions::default(), assets);
    engine_to_pdf(&engine)
}

/// Import a structured HTML document and typeset it to PDF in one call —
/// [`import_html`] followed by [`compile_imported`].
pub fn typeset_html(
    html: &str,
    resolver: &dyn AssetResolver,
    config: &TypesetConfig,
) -> Result<Vec<u8>, TypesetError> {
    compile_imported(&import_html(html, resolver), config)
}

/// Resolve a sensible default serif when no body font is chosen, so output isn't
/// a monospace fallback. Headings left empty inherit the body font.
fn effective_config(config: &TypesetConfig) -> TypesetConfig {
    let mut effective = config.clone();
    if effective.body_font.family.trim().is_empty() {
        let families = available_font_families();
        if let Some(serif) = DEFAULT_SERIF_PREFERENCES
            .iter()
            .find(|pref| families.iter().any(|f| f.eq_ignore_ascii_case(pref)))
        {
            effective.body_font.family = (*serif).to_string();
        }
    }
    effective
}

/// Build a Typst engine for `source` with the given font search options and any
/// in-memory `assets` (math SVGs, fetched images) registered as files. Shared by
/// full PDF compilation (system + embedded fonts) and math-fragment validation
/// (embedded only, for speed), so both go through one builder.
pub(crate) fn build_engine(
    source: String,
    fonts: TypstKitFontOptions,
    assets: &[(String, Vec<u8>)],
) -> TypstEngine<TypstTemplateMainFile> {
    TypstEngine::builder()
        .main_file(source)
        .search_fonts_with(fonts)
        .with_static_file_resolver(assets.iter().map(|(n, b)| (n.as_str(), b.as_slice())))
        .build()
}

/// Compile a built engine to PDF bytes.
fn engine_to_pdf(engine: &TypstEngine<TypstTemplateMainFile>) -> Result<Vec<u8>, TypesetError> {
    let compiled = engine.compile::<PagedDocument>();
    for warning in &compiled.warnings {
        log::debug!("typst warning: {}", warning.message);
    }
    let document = compiled
        .output
        .map_err(|e| TypesetError::Compile(e.to_string()))?;

    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|diags| {
        let msg = diags
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        TypesetError::Pdf(msg)
    })
}

/// Sorted, de-duplicated list of installed system font family names, for a font
/// picker. Falls back to an empty list if the system font set can't be read.
pub fn available_font_families() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _lang)| name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> TypesetInput {
        TypesetInput {
            text: text.to_string(),
            format: InputFormat::Markdown,
        }
    }

    #[test]
    fn markdown_typesets_to_pdf() {
        let input = md("# Title\n\nA paragraph with *bold* and _italic_ text.\n");
        let pdf = typeset(&input, &TypesetConfig::default()).expect("typeset");
        assert!(pdf.starts_with(b"%PDF"), "output should be a PDF");
        assert!(pdf.len() > 500, "PDF unexpectedly tiny");
    }

    #[test]
    fn page_break_rule_adds_pages() {
        let cfg = TypesetConfig::default(); // default rules include "-----" => Replace
        let one = typeset(&md("Page one."), &cfg).expect("one");
        let two = typeset(&md("Page one.\n\n-----\n\nPage two."), &cfg).expect("two");
        // Forcing a page break should produce a larger document.
        assert!(two.len() > one.len(), "page break should grow the PDF");
    }

    #[test]
    fn body_font_size_is_applied() {
        // The same text at a much larger body size must reflow to more pages,
        // proving the size actually reaches the Typst output.
        let text = "word ".repeat(400);
        let input = md(&text);

        let small = TypesetConfig {
            body_font: FontChoice::new("", 8.0),
            ..TypesetConfig::default()
        };
        let large = TypesetConfig {
            body_font: FontChoice::new("", 30.0),
            ..TypesetConfig::default()
        };

        let small_pdf = typeset(&input, &small).expect("small");
        let large_pdf = typeset(&input, &large).expect("large");
        assert!(
            large_pdf.len() > small_pdf.len(),
            "larger body size should produce a larger (more pages) PDF"
        );
    }

    #[test]
    fn plaintext_escapes_specials() {
        let input = TypesetInput {
            text: "Costs $5 #1 *not bold* _not italic_".to_string(),
            format: InputFormat::Plaintext,
        };
        let pdf = typeset(&input, &TypesetConfig::default()).expect("typeset");
        assert!(pdf.starts_with(b"%PDF"));
    }
}
