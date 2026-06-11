//! The `arXiv` e-print source archive, used to upgrade figures to print
//! resolution.
//!
//! `LaTeXML` rasterizes vector graphics (PDF/EPS/PS) to modest-resolution PNGs
//! when it produces the HTML rendering: a converted graphic loses its name and
//! becomes `xN.png`, numbered in document order, while already-raster graphics
//! keep their source path. The vector originals still live in the paper's
//! e-print tarball, so for every `xN.png` (matched by conversion order against
//! the source's `\includegraphics` commands) or same-stem vector sibling we
//! re-rasterize the original PDF at print resolution with `PDFium`. Raster
//! originals (photos, screenshots) have no better version and are left alone.

use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::sync::LazyLock;

use regex::Regex;

/// Rasterization target for vector figures, relative to the figure's own page
/// size. Deliberately above print resolution (300 DPI): the final layout
/// usually scales a figure up to the full text width, so headroom here is what
/// keeps the *effective* resolution on the page at print grade.
const TARGET_DPI: f32 = 600.0;
/// Cap on the longest rendered edge, bounding memory on poster-size figures.
const MAX_EDGE_PX: f32 = 4500.0;
/// Cap on the downloaded archive size.
const MAX_ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;

/// `\input{...}` / `\include{...}` and `\includegraphics[...]{...}`, matched
/// together so graphics are collected in true document order across files.
static TEX_REF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\(?:(input|include)\{([^}]+)\}|includegraphics\s*(?:\[[^\]]*\])?\s*\{([^}]+)\})")
        .unwrap()
});

/// An unpacked e-print source archive.
pub(crate) struct SourceArchive {
    /// File contents keyed by archive path (normalized, no leading `./`).
    files: HashMap<String, Vec<u8>>,
    /// Archive paths of `\includegraphics` targets `LaTeXML` converts (vector
    /// formats), in document order — index N−1 pairs with `xN.png`.
    converted: Vec<String>,
}

impl SourceArchive {
    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }

    pub(crate) fn vector_figure_count(&self) -> usize {
        self.converted.len()
    }

    /// Print-resolution replacement bytes (PNG) for the `LaTeXML` image at
    /// source-relative `rel`, if its vector original can be found and
    /// rasterized. EPS/PS originals are skipped — `PDFium` only reads PDF.
    pub(crate) fn upgrade(&self, rel: &str) -> Option<Vec<u8>> {
        let original = if let Some(n) = converted_index(rel) {
            self.converted.get(n)?
        } else {
            &self.vector_sibling(rel)?
        };
        if !has_ext(original, "pdf") {
            return None;
        }
        rasterize_pdf(self.files.get(original)?)
    }

    /// A vector file sharing `rel`'s path stem (e.g. `Figures/f.png` next to a
    /// source `Figures/f.pdf`).
    fn vector_sibling(&self, rel: &str) -> Option<String> {
        let stem = rel.rsplit_once('.').map_or(rel, |(s, _)| s);
        [".pdf", ".eps", ".ps"]
            .iter()
            .map(|ext| format!("{stem}{ext}"))
            .find(|p| self.files.contains_key(p))
    }
}

/// The 0-based conversion index of a `LaTeXML`-generated graphic name:
/// `x12.png` → `11`. Anything else is not a converted graphic.
fn converted_index(rel: &str) -> Option<usize> {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name.strip_prefix('x')?
        .strip_suffix(".png")?
        .parse::<usize>()
        .ok()?
        .checked_sub(1)
}

/// Fetch and index the e-print source for an `arXiv` id.
pub(crate) fn fetch_archive(id: &str) -> Result<SourceArchive, String> {
    let url = format!("https://arxiv.org/e-print/{id}");
    let mut response = ureq::get(&url).call().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_ARCHIVE_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let files = unpack(&bytes)?;
    let converted = converted_graphics(&files);
    Ok(SourceArchive { files, converted })
}

/// Unpack an e-print download: usually a gzipped tar, sometimes a single
/// gzipped `.tex`, and a bare PDF when the author submitted no source.
fn unpack(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    if bytes.starts_with(b"%PDF") {
        return Err("the paper has no LaTeX source (PDF-only submission)".to_string());
    }
    let data = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(bytes)
            .take(MAX_ARCHIVE_BYTES)
            .read_to_end(&mut out)
            .map_err(|e| format!("gunzip: {e}"))?;
        out
    } else {
        bytes.to_vec()
    };

    // A tar archive carries "ustar" at offset 257; otherwise it's one tex file.
    if data.len() > 262 && &data[257..262] == b"ustar" {
        let mut files = HashMap::new();
        let mut tar = tar::Archive::new(data.as_slice());
        for entry in tar.entries().map_err(|e| format!("tar: {e}"))? {
            let mut entry = entry.map_err(|e| format!("tar entry: {e}"))?;
            if !entry.header().entry_type().is_file() {
                continue;
            }
            let Ok(path) = entry.path() else { continue };
            let name = normalize(&path.to_string_lossy());
            let mut contents = Vec::new();
            if entry.read_to_end(&mut contents).is_ok() {
                files.insert(name, contents);
            }
        }
        Ok(files)
    } else {
        Ok(HashMap::from([("main.tex".to_string(), data)]))
    }
}

/// Strip a leading `./` and unify separators.
fn normalize(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}

/// Case-insensitive extension check (archives occasionally carry `.PDF`).
fn has_ext(name: &str, ext: &str) -> bool {
    name.rsplit('.')
        .next()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// The `\includegraphics` targets `LaTeXML` would convert to `xN.png` (vector
/// formats), in document order: starting from the main file, following
/// `\input`/`\include`, skipping comments. Raster graphics keep their names in
/// the HTML, so they don't consume an `xN` number.
fn converted_graphics(files: &HashMap<String, Vec<u8>>) -> Vec<String> {
    let main = files
        .iter()
        .filter(|(name, _)| has_ext(name, "tex"))
        .find(|(_, bytes)| {
            String::from_utf8_lossy(bytes).contains("\\documentclass")
        })
        .map(|(name, _)| name.clone())
        .or_else(|| {
            let mut texs: Vec<&String> =
                files.keys().filter(|n| has_ext(n, "tex")).collect();
            texs.sort();
            (texs.len() == 1).then(|| texs[0].clone())
        });
    let mut converted = Vec::new();
    if let Some(main) = main {
        let mut visited = HashSet::new();
        walk_tex(&main, files, &mut converted, &mut visited);
    }
    converted
}

/// Collect converted graphics from one tex file, recursing into its inputs.
fn walk_tex(
    name: &str,
    files: &HashMap<String, Vec<u8>>,
    converted: &mut Vec<String>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(name.to_string()) {
        return;
    }
    let Some(bytes) = files.get(name) else { return };
    let text = String::from_utf8_lossy(bytes);
    for line in text.lines() {
        let line = strip_comment(line);
        for caps in TEX_REF.captures_iter(line) {
            if let Some(input) = caps.get(2) {
                let mut child = normalize(input.as_str().trim());
                if !has_ext(&child, "tex") {
                    child.push_str(".tex");
                }
                walk_tex(&child, files, converted, visited);
            } else if let Some(graphic) = caps.get(3)
                && let Some(path) = resolve_graphic(graphic.as_str(), files)
                && matches!(path.rsplit('.').next(), Some("pdf" | "eps" | "ps"))
            {
                converted.push(path);
            }
        }
    }
}

/// Cut a `TeX` line at its first unescaped `%`.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'%' => return &line[..i],
            _ => i += 1,
        }
    }
    line
}

/// Resolve an `\includegraphics` argument against the archive: as given, then
/// with the extensions `latex`/`pdflatex` would try.
fn resolve_graphic(arg: &str, files: &HashMap<String, Vec<u8>>) -> Option<String> {
    let arg = normalize(arg.trim());
    if files.contains_key(&arg) {
        return Some(arg);
    }
    [".pdf", ".png", ".jpg", ".jpeg", ".eps", ".ps"]
        .iter()
        .map(|ext| format!("{arg}{ext}"))
        .find(|p| files.contains_key(p))
}

/// Rasterize page 1 of a PDF figure to PNG at print resolution (capped so a
/// huge page can't allocate unbounded memory).
fn rasterize_pdf(bytes: &[u8]) -> Option<Vec<u8>> {
    // Probe the native size first so the scale can be capped before the real
    // render; figure pages are small, so the probe render is cheap.
    let (_, (w_pts, h_pts)) = junk_libs_platen::render_page_bitmap_from_bytes(bytes, 0, 0.05).ok()?;
    let scale = (TARGET_DPI / 72.0).min(MAX_EDGE_PX / w_pts.max(h_pts).max(1.0));
    let (image, _) = junk_libs_platen::render_page_bitmap_from_bytes(bytes, 0, scale).ok()?;

    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(files: &[(&str, &str)]) -> SourceArchive {
        let files: HashMap<String, Vec<u8>> = files
            .iter()
            .map(|(n, c)| ((*n).to_string(), c.as_bytes().to_vec()))
            .collect();
        let converted = converted_graphics(&files);
        SourceArchive { files, converted }
    }

    #[test]
    fn graphics_collected_in_document_order_across_inputs() {
        let a = archive(&[
            (
                "ms.tex",
                r"\documentclass{article}
\input{intro}
\includegraphics{Figures/photo} % raster: keeps its name
\input{vis}
",
            ),
            ("intro.tex", r"\includegraphics[width=\textwidth]{./fig/a.pdf}"),
            (
                "vis.tex",
                "% \\includegraphics{fig/commented.pdf}\n\\includegraphics{fig/b}\n",
            ),
            ("fig/a.pdf", "%PDF-"),
            ("fig/b.pdf", "%PDF-"),
            ("fig/commented.pdf", "%PDF-"),
            ("Figures/photo.png", "PNG"),
        ]);
        // Only vector graphics consume xN numbers; comments are skipped; the
        // extension-less reference resolves to its .pdf.
        assert_eq!(a.converted, vec!["fig/a.pdf", "fig/b.pdf"]);
        assert_eq!(a.vector_figure_count(), 2);
    }

    #[test]
    fn xn_names_map_to_conversion_order() {
        assert_eq!(converted_index("x1.png"), Some(0));
        assert_eq!(converted_index("sub/x12.png"), Some(11));
        assert_eq!(converted_index("Figures/photo.png"), None);
        assert_eq!(converted_index("x.png"), None);
    }

    #[test]
    fn vector_sibling_matches_stem() {
        let a = archive(&[
            ("ms.tex", r"\documentclass{a}"),
            ("Figures/f.pdf", "%PDF-"),
        ]);
        assert_eq!(a.vector_sibling("Figures/f.png").as_deref(), Some("Figures/f.pdf"));
        assert_eq!(a.vector_sibling("Figures/other.png"), None);
    }

    #[test]
    fn single_tex_unpack_and_pdf_only_rejection() {
        assert!(unpack(b"%PDF-1.5 ...").is_err());
        let files = unpack(b"\\documentclass{article}").unwrap();
        assert!(files.contains_key("main.tex"));
    }

    #[test]
    fn comments_are_stripped_but_escaped_percent_kept() {
        assert_eq!(strip_comment(r"a % b"), "a ");
        assert_eq!(strip_comment(r"100\% sure % note"), r"100\% sure ");
    }

    /// Full upgrade path: a real (typeset) PDF original is rasterized to a PNG
    /// larger than `LaTeXML`'s preview-resolution rendering would be.
    #[test]
    fn upgrades_xn_to_print_resolution_png() {
        let pdf = pdf_typeset::typeset(
            &pdf_typeset::TypesetInput {
                text: "Figure".to_string(),
                format: pdf_typeset::InputFormat::Plaintext,
            },
            &pdf_typeset::TypesetConfig::default(),
        )
        .expect("a small PDF figure");

        let mut files = HashMap::new();
        files.insert(
            "ms.tex".to_string(),
            b"\\documentclass{a}\\includegraphics{fig/plot.pdf}".to_vec(),
        );
        files.insert("fig/plot.pdf".to_string(), pdf);
        let converted = converted_graphics(&files);
        let archive = SourceArchive { files, converted };

        let png = archive.upgrade("x1.png").expect("rasterized PNG");
        assert!(png.starts_with(b"\x89PNG"), "PNG magic");
        let img = image::load_from_memory(&png).expect("decodable");
        // The default (A5) page at 300 DPI is ~1750 px wide — far beyond
        // LaTeXML's ~600 px preview rasterizations.
        assert!(img.width() > 1500, "print resolution, got {}", img.width());
    }
}
