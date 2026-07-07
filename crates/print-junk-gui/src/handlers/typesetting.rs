//! Worker-side handlers for the typesetting mode. Typst compilation is CPU-bound,
//! so it runs on a blocking task rather than the async worker thread.

use std::path::PathBuf;
use std::sync::Arc;

use pdf_async_runtime::{
    InputFormat, PdfUpdate, SectionOverrides, SharedAssets, SharedOutline, TypesetConfig,
    TypesetInput,
};
use tokio::sync::mpsc;

fn count_pdf_pages(bytes: &[u8]) -> usize {
    lopdf::Document::load_mem(bytes).map_or(0, |doc| doc.get_pages().len())
}

/// Convert a raw import payload with the importer its `kind` selects. `smart`
/// applies only to Markdown (typographic dashes/ellipses); HTML ignores it.
fn convert(
    kind: InputFormat,
    payload: &str,
    resolver: &dyn pdf_typeset::AssetResolver,
    smart: bool,
) -> pdf_typeset::ImportedDoc {
    match kind {
        InputFormat::Markdown => pdf_typeset::import_markdown(payload, resolver, smart),
        _ => pdf_typeset::import_html(payload, resolver),
    }
}

fn send_error(update_tx: &mpsc::UnboundedSender<PdfUpdate>, message: String) {
    let _ = update_tx.send(PdfUpdate::Error { message });
}

pub async fn handle_generate_preview(
    input: TypesetInput,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match tokio::task::spawn_blocking(move || pdf_typeset::typeset(&input, &config)).await {
        Ok(Ok(pdf_bytes)) => {
            let page_count = count_pdf_pages(&pdf_bytes);
            let _ = update_tx.send(PdfUpdate::TypesetPreviewGenerated {
                pdf_bytes,
                page_count,
            });
        }
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

pub async fn handle_generate(
    input: TypesetInput,
    config: TypesetConfig,
    output_path: PathBuf,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match tokio::task::spawn_blocking(move || pdf_typeset::typeset(&input, &config)).await {
        Ok(Ok(pdf_bytes)) => match tokio::fs::write(&output_path, &pdf_bytes).await {
            Ok(()) => {
                let _ = update_tx.send(PdfUpdate::TypesetComplete { path: output_path });
            }
            Err(e) => send_error(update_tx, format!("Failed to write PDF: {e}")),
        },
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

/// Acquire a document, convert it once (capturing its fetched assets), and
/// compile a preview. Returns both the raw payload to persist and the converted
/// artifact to cache in-memory for cheap recompiles.
pub async fn handle_import(
    source: String,
    mut config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let task = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let imported = pdf_import::fetch(&source).map_err(|e| format!("Import failed: {e}"))?;
        // Capture the assets the importer fetches so they can be cached offline.
        let cap = pdf_typeset::CapturingResolver::new(&imported);
        let doc = convert(
            imported.kind,
            &imported.payload,
            &cap,
            config.smart_punctuation,
        );
        let raw_assets = cap.into_assets();
        // The import emits content only; the extracted title seeds the template's
        // title page unless the user already chose one. The UI mirrors this same
        // defaulting when it receives `TypesetImported`, so later recompiles match.
        if config.doc_title.trim().is_empty()
            && let Some(title) = &doc.title
        {
            config.doc_title.clone_from(title);
        }
        let pdf = pdf_typeset::compile_imported(&doc, &config)
            .map_err(|e| format!("Typesetting failed: {e}"))?;
        // Read after conversion — figure upgrades are counted as they resolve.
        let asset_report = imported.asset_report();
        Ok((
            source,
            imported.kind,
            imported.payload,
            raw_assets,
            doc,
            pdf,
            asset_report,
        ))
    });
    match task.await {
        Ok(Ok((source, kind, payload, raw_assets, doc, pdf_bytes, asset_report))) => {
            let page_count = count_pdf_pages(&pdf_bytes);
            let _ = update_tx.send(PdfUpdate::TypesetImported {
                pdf_bytes,
                page_count,
                source,
                payload: Arc::new(payload),
                kind,
                raw_assets: Arc::new(raw_assets),
                body: Arc::new(doc.body),
                assets: Arc::new(doc.assets),
                outline: Arc::new(doc.outline),
                title: doc.title,
                stats: doc.stats,
                asset_report,
            });
        }
        Ok(Err(msg)) => send_error(update_tx, msg),
        Err(e) => send_error(update_tx, format!("Import task panicked: {e}")),
    }
}

/// Re-convert a cached import from its raw HTML + assets (offline) and compile a
/// preview — the restore path, which also refreshes the in-memory converted cache.
/// The session's saved section `overrides` are applied to the compile.
pub async fn handle_reconvert(
    payload: Arc<String>,
    kind: InputFormat,
    raw_assets: SharedAssets,
    overrides: SectionOverrides,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let task = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let resolver = pdf_typeset::MapResolver::new(raw_assets.iter().cloned());
        let doc = convert(kind, &payload, &resolver, config.smart_punctuation);
        let assembled = pdf_typeset::assemble_body(&doc.body, &doc.outline, &overrides);
        let pdf = pdf_typeset::compile_body(&assembled, &doc.assets, &config)
            .map_err(|e| format!("Typesetting failed: {e}"))?;
        Ok((doc, pdf))
    });
    match task.await {
        Ok(Ok((doc, pdf_bytes))) => {
            let page_count = count_pdf_pages(&pdf_bytes);
            let _ = update_tx.send(PdfUpdate::TypesetReconverted {
                pdf_bytes,
                page_count,
                body: Arc::new(doc.body),
                assets: Arc::new(doc.assets),
                outline: Arc::new(doc.outline),
                title: doc.title,
                stats: doc.stats,
            });
        }
        Ok(Err(msg)) => send_error(update_tx, msg),
        Err(e) => send_error(update_tx, format!("Reconvert task panicked: {e}")),
    }
}

/// Recompile an already-converted import (cached `body` + `assets`) to a preview.
/// The cheap path taken on settings changes — no network, no re-conversion, and
/// the `Arc`s avoid copying asset bytes. Section `overrides` are applied as a
/// string pass over the cached body.
pub async fn handle_compile_imported(
    body: Arc<String>,
    assets: SharedAssets,
    outline: SharedOutline,
    overrides: SectionOverrides,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let task = tokio::task::spawn_blocking(move || {
        let assembled = pdf_typeset::assemble_body(&body, &outline, &overrides);
        pdf_typeset::compile_body(&assembled, &assets, &config)
    });
    match task.await {
        Ok(Ok(pdf_bytes)) => {
            let page_count = count_pdf_pages(&pdf_bytes);
            let _ = update_tx.send(PdfUpdate::TypesetPreviewGenerated {
                pdf_bytes,
                page_count,
            });
        }
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

/// Write a freshly typeset PDF to a unique temp file and signal the imposition
/// mode to pick it up. The imposition pipeline is path-based, so a temp file is
/// the cleanest handoff.
async fn send_pdf_to_impose(pdf_bytes: Vec<u8>, update_tx: &mpsc::UnboundedSender<PdfUpdate>) {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = std::env::temp_dir().join(format!("print-junk-typeset-{unique}.pdf"));
    match tokio::fs::write(&path, &pdf_bytes).await {
        Ok(()) => {
            let _ = update_tx.send(PdfUpdate::TypesetReadyForImpose { path });
        }
        Err(e) => send_error(update_tx, format!("Failed to write PDF: {e}")),
    }
}

pub async fn handle_send_to_impose(
    input: TypesetInput,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    match tokio::task::spawn_blocking(move || pdf_typeset::typeset(&input, &config)).await {
        Ok(Ok(pdf_bytes)) => send_pdf_to_impose(pdf_bytes, update_tx).await,
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

/// Compile a converted import (cached `body` + `assets`) and write it to `output_path`.
pub async fn handle_generate_imported(
    body: Arc<String>,
    assets: SharedAssets,
    outline: SharedOutline,
    overrides: SectionOverrides,
    config: TypesetConfig,
    output_path: PathBuf,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let task = tokio::task::spawn_blocking(move || {
        let assembled = pdf_typeset::assemble_body(&body, &outline, &overrides);
        pdf_typeset::compile_body(&assembled, &assets, &config)
    });
    match task.await {
        Ok(Ok(pdf_bytes)) => match tokio::fs::write(&output_path, &pdf_bytes).await {
            Ok(()) => {
                let _ = update_tx.send(PdfUpdate::TypesetComplete { path: output_path });
            }
            Err(e) => send_error(update_tx, format!("Failed to write PDF: {e}")),
        },
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

/// Compile a converted import and hand the result to the imposition mode.
pub async fn handle_send_imported_to_impose(
    body: Arc<String>,
    assets: SharedAssets,
    outline: SharedOutline,
    overrides: SectionOverrides,
    config: TypesetConfig,
    update_tx: &mpsc::UnboundedSender<PdfUpdate>,
) {
    let task = tokio::task::spawn_blocking(move || {
        let assembled = pdf_typeset::assemble_body(&body, &outline, &overrides);
        pdf_typeset::compile_body(&assembled, &assets, &config)
    });
    match task.await {
        Ok(Ok(pdf_bytes)) => send_pdf_to_impose(pdf_bytes, update_tx).await,
        Ok(Err(e)) => send_error(update_tx, format!("Typesetting failed: {e}")),
        Err(e) => send_error(update_tx, format!("Typesetting task panicked: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    /// End-to-end import handler check: acquire a local document, convert it
    /// (capturing its fetched image), compile, and emit a `TypesetImported`
    /// update carrying the raw payload + converted artifact + stats.
    #[tokio::test]
    async fn import_handler_fetches_converts_and_compiles() {
        const HTML: &str = r##"<html><body><article class="ltx_document">
            <h1 class="ltx_title ltx_title_document">Test</h1>
            <p class="ltx_p">See<cite class="ltx_cite"><a href="#bib.bib1">1</a></cite> and
              <math><semantics><annotation encoding="application/x-tex">a+b</annotation></semantics></math>.
              <img src="fig.png"></p>
            <ol class="ltx_biblist"><li id="bib.bib1" class="ltx_bibitem">
              <span class="ltx_tag">[1]</span><span class="ltx_bibblock">A. Author. Title. 2020.</span>
            </li></ol></article></body></html>"##;
        // A valid 1×1 PNG so Typst can embed the figure during compilation.
        let png = base64::engine::general_purpose::STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DAAAAEAQEARwbK3gAAAABJRU5ErkJggg==")
            .unwrap();

        let dir = std::env::temp_dir().join(format!("pj-import-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("doc.html"), HTML).unwrap();
        std::fs::write(dir.join("fig.png"), &png).unwrap();
        let source = dir.join("doc.html").to_string_lossy().into_owned();

        let (tx, mut rx) = mpsc::unbounded_channel();
        handle_import(source, TypesetConfig::default(), &tx).await;
        let update = rx.try_recv().expect("an update was sent");
        std::fs::remove_dir_all(&dir).ok();

        match update {
            PdfUpdate::TypesetImported {
                pdf_bytes,
                page_count,
                raw_assets,
                body,
                stats,
                ..
            } => {
                assert!(!pdf_bytes.is_empty() && page_count >= 1, "valid PDF");
                assert_eq!(stats.citations, 1, "one citation linked");
                assert!(stats.math_tex >= 1, "native math converted");
                assert!(
                    raw_assets.iter().any(|(n, _)| n == "fig.png"),
                    "the fetched image was captured for caching"
                );
                assert!(
                    body.contains("#link(<bib-bib1>)"),
                    "citation links to the bib label: {body}"
                );
            }
            other => panic!("expected TypesetImported, got {other:?}"),
        }
    }
}
