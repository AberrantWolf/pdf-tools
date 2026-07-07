//! Acquire an HTML document from a URL, an `arXiv` id/URL, or a local file, and
//! resolve its image assets.
//!
//! This is the I/O layer for the structured importer in `pdf-typeset`: it fetches
//! the markup and provides an [`AssetResolver`] so `pdf-typeset` itself stays
//! network-free. `arXiv` references are normalized to their HTML rendering
//! (native `arxiv.org/html`, falling back to `ar5iv`), since the abstract page
//! carries no full text.
//!
//! With the `hires` feature, an `arXiv` import also fetches the paper's e-print
//! source archive and re-rasterizes vector figures at print resolution (see
//! [`archive`]); the [`AssetReport`] tells the caller what happened.

#[cfg(feature = "hires")]
mod archive;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use pdf_typeset::{AssetResolver, InputFormat};
use regex::Regex;
use serde::{Deserialize, Serialize};

static NEW_FIND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{4}\.\d{4,5}(?:v\d+)?").unwrap());
static OLD_FIND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-z][a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?").unwrap());
static NEW_FULL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}\.\d{4,5}(?:v\d+)?$").unwrap());
static OLD_FULL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?$").unwrap());

/// Errors from acquiring a document.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("fetch failed: {0}")]
    Fetch(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid source: {0}")]
    Invalid(String),
}

/// How fetching the `arXiv` e-print source archive (for hi-res figures) went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchiveStatus {
    /// Not an `arXiv` import — there is no source archive to fetch.
    NotApplicable,
    /// Built without the `hires` feature; no fetch was attempted.
    Disabled,
    /// The `LaTeX` source was fetched and indexed.
    Fetched {
        files: usize,
        /// Vector figures found in the source — the upgrade candidates.
        vector_figures: usize,
    },
    /// Fetching or unpacking failed (network, PDF-only paper) — retryable.
    Failed(String),
}

/// What the asset pipeline did during an import, for status display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetReport {
    pub archive: ArchiveStatus,
    /// Figures replaced with a print-resolution rasterization of their vector
    /// original.
    pub figures_upgraded: usize,
}

/// An acquired document: its raw payload plus the base it was loaded from.
/// Implements [`AssetResolver`] so it can be handed straight to the matching
/// `pdf_typeset` importer. The payload is HTML or Markdown per [`Imported::kind`].
pub struct Imported {
    pub payload: String,
    /// Which importer the payload feeds (HTML for URLs/arXiv, Markdown for a
    /// local `.md` file).
    pub kind: InputFormat,
    /// The resolved URL the payload came from (`None` for a local file).
    pub source_url: Option<String>,
    base: Base,
    #[cfg(feature = "hires")]
    archive: Option<archive::SourceArchive>,
    archive_status: ArchiveStatus,
    /// Count of hi-res figure upgrades served, recorded during resolution.
    upgraded: std::cell::Cell<usize>,
}

impl Imported {
    /// What the asset pipeline did — call after the conversion has resolved its
    /// images, since upgrades are counted as they are served.
    pub fn asset_report(&self) -> AssetReport {
        AssetReport {
            archive: self.archive_status.clone(),
            figures_upgraded: self.upgraded.get(),
        }
    }

    fn new(payload: String, kind: InputFormat, source_url: Option<String>, base: Base) -> Self {
        Self {
            payload,
            kind,
            source_url,
            base,
            #[cfg(feature = "hires")]
            archive: None,
            archive_status: ArchiveStatus::NotApplicable,
            upgraded: std::cell::Cell::new(0),
        }
    }
}

enum Base {
    Url(url::Url),
    Dir(PathBuf),
}

/// Acquire a document from `source`: a local file path, an `arXiv` id/abs/pdf
/// URL, or a generic URL.
pub fn fetch(source: &str) -> Result<Imported, ImportError> {
    let source = source.trim();

    // 1. Local file. A `.md` file feeds the Markdown importer; anything else is
    // treated as HTML.
    let path = Path::new(source);
    if path.exists() {
        let payload = std::fs::read_to_string(path)?;
        let dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let kind = match path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(InputFormat::from_extension)
        {
            Some(InputFormat::Markdown) => InputFormat::Markdown,
            _ => InputFormat::Html,
        };
        return Ok(Imported::new(payload, kind, None, Base::Dir(dir)));
    }

    // 2. arXiv reference -> HTML rendering (native, then ar5iv fallback).
    if let Some(id) = arxiv_id(source) {
        let mut last = String::new();
        for url in [
            format!("https://arxiv.org/html/{id}"),
            format!("https://ar5iv.labs.arxiv.org/html/{id}"),
        ] {
            match get(&url) {
                Ok(html) if looks_like_document(&html) => {
                    let base =
                        url::Url::parse(&url).map_err(|e| ImportError::Invalid(e.to_string()))?;
                    let mut imported =
                        Imported::new(html, InputFormat::Html, Some(url), Base::Url(base));
                    imported.attach_archive(&id);
                    return Ok(imported);
                }
                Ok(_) => last = format!("{url}: no document content"),
                Err(e) => last = format!("{url}: {e}"),
            }
        }
        return Err(ImportError::Fetch(format!(
            "no HTML version for arXiv {id} ({last})"
        )));
    }

    // 3. Generic URL.
    let base = url::Url::parse(source)
        .map_err(|_| ImportError::Invalid(format!("not a file, arXiv id, or URL: {source}")))?;
    let html = get(source)?;
    Ok(Imported::new(
        html,
        InputFormat::Html,
        Some(source.to_string()),
        Base::Url(base),
    ))
}

impl Imported {
    /// Try to fetch the e-print source archive for hi-res figure upgrades. A
    /// failure is recorded for status display, never fatal — the HTML's own
    /// images still resolve.
    #[cfg(feature = "hires")]
    fn attach_archive(&mut self, id: &str) {
        match archive::fetch_archive(id) {
            Ok(a) => {
                log::info!(
                    "arXiv {id}: source archive fetched ({} files, {} vector figures)",
                    a.file_count(),
                    a.vector_figure_count()
                );
                self.archive_status = ArchiveStatus::Fetched {
                    files: a.file_count(),
                    vector_figures: a.vector_figure_count(),
                };
                self.archive = Some(a);
            }
            Err(e) => {
                log::warn!("arXiv {id}: no source archive for hi-res figures: {e}");
                self.archive_status = ArchiveStatus::Failed(e);
            }
        }
    }

    #[cfg(not(feature = "hires"))]
    fn attach_archive(&mut self, _id: &str) {
        self.archive_status = ArchiveStatus::Disabled;
    }

    /// Print-resolution bytes for `src`, when its vector original is in the
    /// archive. The HTML's image paths are document-relative with a leading id
    /// segment (`<id>/Figures/f.png`), so both the full path and the part after
    /// the first segment are tried against the source tree.
    #[cfg(feature = "hires")]
    fn upgraded_bytes(&self, src: &str) -> Option<Vec<u8>> {
        let archive = self.archive.as_ref()?;
        let path = src.split(['?', '#']).next().unwrap_or(src);
        let rel = path.split_once('/').map_or(path, |(_, rest)| rest);
        archive.upgrade(path).or_else(|| archive.upgrade(rel))
    }
}

fn looks_like_document(html: &str) -> bool {
    html.contains("ltx_document") || html.contains("application/x-tex")
}

fn get(url: &str) -> Result<String, ImportError> {
    ureq::get(url)
        .call()
        .map_err(|e| ImportError::Fetch(e.to_string()))?
        .body_mut()
        .read_to_string()
        .map_err(|e| ImportError::Fetch(e.to_string()))
}

/// Extract an `arXiv` id from a URL or bare reference, if `source` is one.
fn arxiv_id(source: &str) -> Option<String> {
    if source.contains("arxiv.org") || source.contains("ar5iv") {
        return NEW_FIND
            .find(source)
            .or_else(|| OLD_FIND.find(source))
            .map(|m| m.as_str().to_string());
    }
    if !source.contains("://") {
        let cand = source.trim_end_matches(".pdf");
        if NEW_FULL.is_match(cand) || OLD_FULL.is_match(cand) {
            return Some(cand.to_string());
        }
    }
    None
}

impl AssetResolver for Imported {
    fn fetch(&self, src: &str) -> Option<Vec<u8>> {
        #[cfg(feature = "hires")]
        if let Some(bytes) = self.upgraded_bytes(src) {
            self.upgraded.set(self.upgraded.get() + 1);
            return Some(bytes);
        }
        match &self.base {
            Base::Url(base) => {
                let abs = base.join(src).ok()?;
                ureq::get(abs.as_str())
                    .call()
                    .ok()?
                    .body_mut()
                    .read_to_vec()
                    .ok()
            }
            Base::Dir(dir) => {
                if src.contains("://") {
                    return None;
                }
                std::fs::read(dir.join(src)).ok()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{arxiv_id, fetch};
    use pdf_typeset::InputFormat;

    #[test]
    fn recognizes_arxiv_forms() {
        assert_eq!(arxiv_id("2310.12345").as_deref(), Some("2310.12345"));
        assert_eq!(arxiv_id("2310.12345v2").as_deref(), Some("2310.12345v2"));
        assert_eq!(
            arxiv_id("https://arxiv.org/abs/1706.03762").as_deref(),
            Some("1706.03762")
        );
        assert_eq!(
            arxiv_id("arxiv.org/pdf/1706.03762v5").as_deref(),
            Some("1706.03762v5")
        );
        assert_eq!(
            arxiv_id("hep-th/9901001").as_deref(),
            Some("hep-th/9901001")
        );
    }

    #[test]
    fn ignores_non_arxiv() {
        assert_eq!(arxiv_id("https://example.com/page.html"), None);
        assert_eq!(arxiv_id("notes.html"), None);
    }

    #[test]
    fn local_markdown_file_fetches_as_markdown() {
        let dir = std::env::temp_dir().join(format!("pdf-import-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.md");
        std::fs::write(&path, "# Heading\n\nBody.\n").unwrap();

        let imported = fetch(path.to_str().unwrap()).expect("fetch");
        assert_eq!(imported.kind, InputFormat::Markdown);
        assert!(imported.payload.contains("# Heading"));

        let html = dir.join("page.html");
        std::fs::write(&html, "<p>hi</p>").unwrap();
        assert_eq!(
            fetch(html.to_str().unwrap()).unwrap().kind,
            InputFormat::Html
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
