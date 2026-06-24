//! Structured HTML → `Typst` import, targeting `LaTeXML` output (`arXiv` HTML and
//! `ar5iv`).
//!
//! Parses the page, isolates the document content (dropping the table of
//! contents, navigation, and page chrome), and walks the DOM into `Typst`
//! markup. `<math>` elements are routed through [`crate::MathPipeline`]; images
//! are fetched through an injected [`AssetResolver`] so this module stays
//! I/O-free and testable from fixtures. Math image fallbacks and fetched images
//! are returned as named assets for the caller to register as `Typst` files.
//!
//! Guiding rule: **never emit a source artifact that we regenerate.** `LaTeXML`
//! bakes presentational markers into the markup — footnote numbers, list
//! bullets, its own title and table of contents — that Typst (or the template)
//! re-creates. Emitting both duplicates them (e.g. a footnote numbered three
//! times, `- •` bullets), so every such marker is stripped here and the
//! regenerated form is the only one that survives.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use scraper::node::Node;
use scraper::{ElementRef, Html, Selector};

use crate::markup::escape_inline;
use crate::outline::{OutlineEntry, SECTION_MARK, strip_markers};
use crate::typst_table::{Align, Cell, Table as TypstTable};
use crate::{MathPipeline, MathSource, Tier};

/// Resolves a referenced asset (an `<img src>`) to its bytes. Implemented by the
/// acquisition layer (HTTP/file); `None` skips the asset.
pub trait AssetResolver {
    fn fetch(&self, src: &str) -> Option<Vec<u8>>;
}

/// A resolver that fetches nothing — useful for tests and text-only imports.
pub struct NoAssets;
impl AssetResolver for NoAssets {
    fn fetch(&self, _src: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Wraps another resolver and records every `(src, bytes)` it successfully
/// yields, so a one-time import's fetched assets can be cached and replayed
/// offline later (see [`MapResolver`]).
pub struct CapturingResolver<'r> {
    inner: &'r dyn AssetResolver,
    captured: std::cell::RefCell<HashMap<String, Vec<u8>>>,
}

impl<'r> CapturingResolver<'r> {
    pub fn new(inner: &'r dyn AssetResolver) -> Self {
        Self {
            inner,
            captured: std::cell::RefCell::new(HashMap::new()),
        }
    }

    /// The assets fetched during the wrapped import, keyed by their `<img src>`.
    pub fn into_assets(self) -> Vec<(String, Vec<u8>)> {
        self.captured.into_inner().into_iter().collect()
    }
}

impl AssetResolver for CapturingResolver<'_> {
    fn fetch(&self, src: &str) -> Option<Vec<u8>> {
        let bytes = self.inner.fetch(src)?;
        self.captured
            .borrow_mut()
            .insert(src.to_string(), bytes.clone());
        Some(bytes)
    }
}

/// Serves assets from an in-memory map — the offline replay of a previously
/// [`CapturingResolver`]-captured import (e.g. on project restore).
pub struct MapResolver {
    assets: HashMap<String, Vec<u8>>,
}

impl MapResolver {
    pub fn new(assets: impl IntoIterator<Item = (String, Vec<u8>)>) -> Self {
        Self {
            assets: assets.into_iter().collect(),
        }
    }
}

impl AssetResolver for MapResolver {
    fn fetch(&self, src: &str) -> Option<Vec<u8>> {
        self.assets.get(src).cloned()
    }
}

/// Counts of how each part of the document was handled, for QA/logging.
#[derive(Debug, Default, Clone, Copy)]
pub struct ImportStats {
    pub math_tex: usize,
    pub math_image: usize,
    pub math_raw: usize,
    pub images_ok: usize,
    pub images_failed: usize,
    pub footnotes: usize,
    pub citations: usize,
}

/// The result of importing an HTML document.
pub struct ImportedDoc {
    /// `Typst` body markup (no template).
    pub body: String,
    /// Named in-memory files (math SVGs, fetched images) to register before
    /// compiling.
    pub assets: Vec<(String, Vec<u8>)>,
    /// One entry per heading, with its byte offset into `body` — drives
    /// per-section overrides (see [`crate::assemble_body`]).
    pub outline: Vec<OutlineEntry>,
    pub title: Option<String>,
    pub stats: ImportStats,
}

/// Import an HTML document into `Typst` markup plus its assets.
///
/// The body is pure content: the document title is extracted into
/// [`ImportedDoc::title`] but *not* emitted, and no outline is injected — front
/// matter (title page, table of contents) is owned by the template via
/// [`crate::TypesetConfig`], so it is never duplicated.
pub fn import(html: &str, resolver: &dyn AssetResolver) -> ImportedDoc {
    let doc = Html::parse_document(html);
    let mut imp = Importer::new(resolver);
    if let Ok(sel) = Selector::parse(".ltx_bibitem") {
        imp.bib_ids = doc
            .select(&sel)
            .filter_map(|e| e.value().attr("id"))
            .map(String::from)
            .collect();
    }
    let raw = content_root(&doc)
        .map(|root| imp.render_children(root))
        .unwrap_or_default();
    let mut outline = imp.outline;
    let body = strip_markers(&raw, &mut outline);

    ImportedDoc {
        body,
        assets: imp.out_assets,
        outline,
        title: imp.title,
        stats: imp.stats,
    }
}

/// Locate the document content container, preferring `LaTeXML`'s.
fn content_root(doc: &Html) -> Option<ElementRef<'_>> {
    for sel in [
        "article.ltx_document",
        ".ltx_document",
        ".ltx_page_content",
        "main",
        "body",
    ] {
        if let Ok(s) = Selector::parse(sel)
            && let Some(e) = doc.select(&s).next()
        {
            return Some(e);
        }
    }
    None
}

/// Elements never emitted: page furniture and the source's own TOC.
fn is_skipped(el: ElementRef<'_>) -> bool {
    const SKIP_TAGS: &[&str] = &[
        "script", "style", "nav", "header", "footer", "button", "form",
    ];
    const SKIP_CLASSES: &[&str] = &[
        "ltx_TOC",
        "ltx_page_navbar",
        "ltx_page_header",
        "ltx_page_footer",
        "ltx_pagination",
        "ltx_rdf",
        // Affiliation reference marks (daggers/numbers) — noise after reflow.
        "ltx_role_footnotemark",
        // Source-rendered markers we regenerate (see the module docs): Typst's
        // `#footnote` numbers the note itself, and list markup draws its own
        // bullets. `ltx_note_content` carries its own mark/tag copies inline.
        "ltx_note_mark",
        "ltx_tag_note",
        "ltx_note_type",
        "ltx_tag_item",
    ];
    let v = el.value();
    SKIP_TAGS.contains(&v.name()) || v.classes().any(|c| SKIP_CLASSES.contains(&c))
}

struct Importer<'r> {
    resolver: &'r dyn AssetResolver,
    math: MathPipeline,
    out_assets: Vec<(String, Vec<u8>)>,
    asset_names: HashMap<String, String>,
    title: Option<String>,
    stats: ImportStats,
    /// `id`s of `<li class="ltx_bibitem">` entries — populated up front so a
    /// citation only emits a `#link` to a label we are certain to render.
    bib_ids: HashSet<String>,
    /// One entry per emitted heading, in document order. Offsets are filled in
    /// by [`strip_markers`] once the recursive render has assembled the body.
    outline: Vec<OutlineEntry>,
    annotation_sel: Selector,
    note_content_sel: Selector,
    tr_sel: Selector,
    cell_sel: Selector,
    math_sel: Selector,
    eqn_tag_sel: Selector,
}

impl<'r> Importer<'r> {
    fn new(resolver: &'r dyn AssetResolver) -> Self {
        Self {
            resolver,
            math: MathPipeline::default(),
            out_assets: Vec::new(),
            asset_names: HashMap::new(),
            title: None,
            stats: ImportStats::default(),
            bib_ids: HashSet::new(),
            outline: Vec::new(),
            annotation_sel: Selector::parse("annotation").unwrap(),
            note_content_sel: Selector::parse(".ltx_note_content").unwrap(),
            tr_sel: Selector::parse("tr").unwrap(),
            cell_sel: Selector::parse("td, th").unwrap(),
            math_sel: Selector::parse("math").unwrap(),
            eqn_tag_sel: Selector::parse(".ltx_tag_equation").unwrap(),
        }
    }

    fn render_children(&mut self, el: ElementRef<'_>) -> String {
        let mut s = String::new();
        for child in el.children() {
            match child.value() {
                Node::Text(t) => s.push_str(&escape_inline(&collapse_ws(&t.text))),
                Node::Element(_) => {
                    if let Some(ce) = ElementRef::wrap(child) {
                        s.push_str(&self.render_element(ce));
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn render_element(&mut self, el: ElementRef<'_>) -> String {
        if is_skipped(el) {
            return String::new();
        }
        let v = el.value();
        if v.classes().any(|c| c == "ltx_role_footnote") {
            return self.render_footnote(el);
        }
        if v.classes().any(|c| c == "ltx_authors") {
            return self.authors(el);
        }
        if v.classes().any(|c| c == "ltx_biblist") {
            return self.bib_list(el);
        }
        // `LaTeXML` font switches arrive as styled spans rather than `<em>`/`<b>`
        // (common in bibliography entries: italic journal/title runs).
        if v.classes().any(|c| c == "ltx_font_italic") {
            return format!("_{}_", self.render_children(el));
        }
        if v.classes().any(|c| c == "ltx_font_bold") {
            return format!("*{}*", self.render_children(el));
        }
        match v.name() {
            "math" => self.render_math(el),
            "h1" => self.render_title_or_heading(el),
            "h2" => self.heading(el, 1),
            "h3" => self.heading(el, 2),
            "h4" => self.heading(el, 3),
            "h5" => self.heading(el, 4),
            "h6" => self.heading(el, 5),
            "p" => format!("{}\n\n", self.render_children(el).trim()),
            "ul" => self.list(el, '-'),
            "ol" => self.list(el, '+'),
            "li" => format!("{}\n", self.render_children(el).trim()),
            // LaTeXML lays display equations out as tables — route those to the
            // math pipeline; only genuine tabular content becomes a Typst table.
            "table"
                if v.classes()
                    .any(|c| c == "ltx_equation" || c == "ltx_equationgroup") =>
            {
                self.equation_table(el)
            }
            "table" => self.table(el),
            "img" => self.image(el),
            "a" => self.link(el),
            "em" | "i" => format!("_{}_", self.render_children(el)),
            "strong" | "b" => format!("*{}*", self.render_children(el)),
            "sup" => format!("#super[{}]", self.render_children(el)),
            "sub" => format!("#sub[{}]", self.render_children(el)),
            "code" | "tt" => format!("#raw({})", typst_string(&el.text().collect::<String>())),
            "br" => "#linebreak() ".to_string(),
            "blockquote" => format!(
                "#quote(block: true)[{}]\n\n",
                self.render_children(el).trim()
            ),
            "figcaption" => {
                format!(
                    "\n#align(center)[#emph[{}]]\n\n",
                    self.render_children(el).trim()
                )
            }
            _ => self.render_children(el),
        }
    }

    fn render_title_or_heading(&mut self, el: ElementRef<'_>) -> String {
        let is_title =
            el.value().classes().any(|c| c == "ltx_title_document") || self.title.is_none();
        if is_title {
            let t = collapse_ws(&el.text().collect::<String>())
                .trim()
                .to_string();
            if !t.is_empty() {
                self.title = Some(t);
            }
            String::new()
        } else {
            self.heading(el, 1)
        }
    }

    /// Emit a heading and record it in the outline. The section id comes from the
    /// nearest enclosing element with an `id` (`LaTeXML` puts stable ids like
    /// `S3.SS2` on section containers), so saved overrides survive re-conversion;
    /// a marker sentinel lets [`strip_markers`] recover the heading's offset.
    fn heading(&mut self, el: ElementRef<'_>, depth: usize) -> String {
        let id = el
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find_map(|a| a.value().attr("id"))
            .map_or_else(|| format!("sec-{}", self.outline.len()), String::from);
        let idx = self.outline.len();
        self.outline.push(OutlineEntry {
            id,
            level: depth.try_into().unwrap_or(u8::MAX),
            title: heading_text(el),
            offset: 0, // recorded by strip_markers
        });
        format!(
            "\n{SECTION_MARK}{idx}{SECTION_MARK}{} {}\n\n",
            "=".repeat(depth),
            self.render_children(el).trim()
        )
    }

    fn list(&mut self, el: ElementRef<'_>, marker: char) -> String {
        let mut s = String::from("\n");
        for child in el.children() {
            if let Some(ce) = ElementRef::wrap(child)
                && ce.value().name() == "li"
            {
                let _ = writeln!(s, "{marker} {}", self.render_children(ce).trim());
            }
        }
        s.push('\n');
        s
    }

    fn link(&mut self, el: ElementRef<'_>) -> String {
        let children = self.render_children(el);
        match el.value().attr("href") {
            Some(href) if href.starts_with('#') => {
                // Internal anchor: link only to a bibliography entry we labelled
                // (a citation). Other anchors won't resolve after reflow, so the
                // text is kept on its own.
                let frag = &href[1..];
                if self.bib_ids.contains(frag) {
                    self.stats.citations += 1;
                    format!("#link(<{}>)[{children}]", sanitize_label(frag))
                } else {
                    children
                }
            }
            Some(href) => format!("#link({})[{children}]", typst_string(href)),
            None => children,
        }
    }

    fn render_math(&mut self, el: ElementRef<'_>) -> String {
        let display = el.value().attr("display") == Some("block");
        let Some(tex) = self.tex_of(el) else {
            return String::new();
        };
        let typst = self.render_tex(&tex, display);
        if display {
            format!("\n\n{typst}\n\n")
        } else {
            typst
        }
    }

    /// The `TeX` source of a `<math>` element, from its `LaTeXML` annotation.
    fn tex_of(&self, el: ElementRef<'_>) -> Option<String> {
        el.select(&self.annotation_sel)
            .find(|a| a.value().attr("encoding") == Some("application/x-tex"))
            .map(|a| a.text().collect::<String>())
    }

    /// Run `TeX` through the math pipeline, recording stats and assets, and return
    /// the `Typst` markup (unwrapped — no display spacing).
    fn render_tex(&mut self, tex: &str, display: bool) -> String {
        let r = self.math.render(&MathSource { tex, display });
        match r.tier {
            Tier::Tex => self.stats.math_tex += 1,
            Tier::Image => self.stats.math_image += 1,
            Tier::Raw => self.stats.math_raw += 1,
        }
        for asset in r.assets {
            self.out_assets.push((asset.name, asset.svg));
        }
        r.typst
    }

    /// The author block, centered. The `\And` separators `LaTeXML` emits as a
    /// literal "&" between authors are removed; affiliation marks are already
    /// skipped via [`is_skipped`].
    fn authors(&mut self, el: ElementRef<'_>) -> String {
        let inner = self.render_children(el).replace('&', "");
        let inner = inner.trim();
        if inner.is_empty() {
            String::new()
        } else {
            format!("\n#align(center)[{inner}]\n\n")
        }
    }

    /// The reference list. Each `ltx_bibitem` becomes a hanging-indent block
    /// carrying a `Typst` label matching its `id`, so the [`Self::link`]-emitted
    /// citations resolve to it. The leading `[N]` tag `LaTeXML` includes is kept
    /// as the visible marker.
    fn bib_list(&mut self, el: ElementRef<'_>) -> String {
        let mut out = String::from("\n");
        for li in el.children().filter_map(ElementRef::wrap) {
            if !li.value().classes().any(|c| c == "ltx_bibitem") {
                continue;
            }
            let body = self.render_children(li);
            let body = body.trim();
            if body.is_empty() {
                continue;
            }
            out.push_str("#block(below: 0.65em)[#par(hanging-indent: 1.5em)[");
            out.push_str(body);
            out.push_str("]]");
            if let Some(id) = li.value().attr("id") {
                let _ = write!(out, " <{}>", sanitize_label(id));
            }
            out.push('\n');
        }
        out.push('\n');
        out
    }

    fn render_footnote(&mut self, el: ElementRef<'_>) -> String {
        self.stats.footnotes += 1;
        let content = el.select(&self.note_content_sel).next();
        let body = match content {
            Some(c) => self.render_children(c),
            None => self.render_children(el),
        };
        format!("#footnote[{}]", body.trim())
    }

    fn image(&mut self, el: ElementRef<'_>) -> String {
        let Some(src) = el.value().attr("src") else {
            return String::new();
        };
        if let Some(name) = self.asset_for(src) {
            format!("#box(image({}))", typst_string(&name))
        } else {
            self.stats.images_failed += 1;
            String::new()
        }
    }

    /// Fetch and register an image asset, returning its `Typst` file name. Deduped
    /// by source so a repeated image is fetched once.
    fn asset_for(&mut self, src: &str) -> Option<String> {
        if let Some(name) = self.asset_names.get(src) {
            return Some(name.clone());
        }
        let bytes = self.resolver.fetch(src)?;
        let name = format!("img-{:016x}{}", hash(src), ext_of(src));
        self.out_assets.push((name.clone(), bytes));
        self.asset_names.insert(src.to_string(), name.clone());
        self.stats.images_ok += 1;
        Some(name)
    }

    /// A `LaTeXML` display equation (`ltx_equation`) or `align` group
    /// (`ltx_equationgroup`), which arrives as a layout `<table>`: each row holds
    /// the equation's math — split into fragments around the alignment point in
    /// groups — plus padding cells and an optional equation-number tag. Rendered
    /// as display math, never as a visible table. The source's equation number is
    /// kept, right-aligned on the same line, so in-text references to "(N)" stay
    /// meaningful. (Cross-row alignment of `align` groups is not reproduced; each
    /// row is centered independently.)
    fn equation_table(&mut self, el: ElementRef<'_>) -> String {
        // Collect each row's TeX and number first — the row walk borrows `self`'s
        // selectors, while rendering needs `&mut self`.
        let rows: Vec<(String, Option<String>)> = el
            .select(&self.tr_sel)
            .filter_map(|tr| {
                let tex = tr
                    .select(&self.math_sel)
                    .filter_map(|m| self.tex_of(m))
                    .collect::<Vec<_>>()
                    .join(" ");
                if tex.trim().is_empty() {
                    return None; // spacer row
                }
                let number = tr
                    .select(&self.eqn_tag_sel)
                    .next()
                    .map(|t| {
                        collapse_ws(&t.text().collect::<String>())
                            .trim()
                            .to_string()
                    })
                    .filter(|n| !n.is_empty());
                Some((tex, number))
            })
            .collect();

        let mut out = String::new();
        for (tex, number) in rows {
            let typst = self.render_tex(&tex, true);
            match number {
                Some(n) => {
                    let _ = write!(
                        out,
                        "\n\n#grid(columns: (1fr, auto), align: (center + horizon, right + horizon), [{typst}], [{}])\n\n",
                        escape_inline(&n)
                    );
                }
                None => {
                    let _ = write!(out, "\n\n{typst}\n\n");
                }
            }
        }
        out
    }

    fn table(&mut self, el: ElementRef<'_>) -> String {
        // Collect cell structure first (this borrows the DOM and `self`'s
        // selectors, not `self` mutably), then render — `render_children` needs
        // `&mut self`. Each raw cell carries its element plus spans/alignment.
        struct RawCell<'a> {
            el: ElementRef<'a>,
            colspan: usize,
            rowspan: usize,
            align: Option<Align>,
        }
        let raw_rows: Vec<(bool, Vec<RawCell<'_>>)> = el
            .select(&self.tr_sel)
            .map(|tr| {
                let cells = tr
                    .select(&self.cell_sel)
                    .map(|c| RawCell {
                        el: c,
                        colspan: span_attr(c, "colspan"),
                        rowspan: span_attr(c, "rowspan"),
                        align: cell_align(c),
                    })
                    .collect();
                (in_thead(tr), cells)
            })
            .filter(|(_, cells): &(bool, Vec<RawCell<'_>>)| !cells.is_empty())
            .collect();

        let mut rows: Vec<Vec<Cell>> = Vec::new();
        let mut header_flags: Vec<bool> = Vec::new();
        // Column alignment is taken from body cells; header cells only fill in
        // columns the body never aligns (header alignment often differs).
        let mut col_aligns: Vec<Option<Align>> = Vec::new();
        let mut header_aligns: Vec<Option<Align>> = Vec::new();
        let mut any_rowspan = false;

        for (is_header, cells) in raw_rows {
            let mut row: Vec<Cell> = Vec::with_capacity(cells.len());
            let mut col = 0usize;
            for c in cells {
                any_rowspan |= c.rowspan > 1;
                if let Some(a) = c.align {
                    let target = if is_header {
                        &mut header_aligns
                    } else {
                        &mut col_aligns
                    };
                    if target.len() <= col {
                        target.resize(col + 1, None);
                    }
                    target[col].get_or_insert(a);
                }
                row.push(Cell {
                    body: self.render_children(c.el).trim().to_string(),
                    colspan: c.colspan,
                    rowspan: c.rowspan,
                });
                col += c.colspan;
            }
            header_flags.push(is_header);
            rows.push(row);
        }

        let columns = rows
            .iter()
            .map(|r| r.iter().map(|c| c.colspan).sum::<usize>())
            .max()
            .unwrap_or(0);
        if columns == 0 {
            return String::new();
        }

        // Pad short rows to keep the grid rectangular — but only when no cell
        // spans rows, since rowspans deliberately leave later rows short and
        // Typst flows around them.
        if !any_rowspan {
            for row in &mut rows {
                let width: usize = row.iter().map(|c| c.colspan).sum();
                for _ in width..columns {
                    row.push(Cell::new(""));
                }
            }
        }

        let aligns = (0..columns)
            .map(|i| {
                col_aligns
                    .get(i)
                    .copied()
                    .flatten()
                    .or_else(|| header_aligns.get(i).copied().flatten())
                    .unwrap_or(Align::Left)
            })
            .collect();
        let header_rows = header_flags.iter().take_while(|h| **h).count();

        TypstTable {
            columns,
            aligns,
            header_rows,
            rows,
        }
        .render()
    }
}

/// Turn an HTML `id` into a `Typst` label name, mapping every non-alphanumeric
/// character to `-`. Applied identically at the citation and bibliography ends so
/// the two always agree (avoids relying on `.`/`:` being valid in labels).
fn sanitize_label(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// A `colspan`/`rowspan` attribute value, clamped to at least 1 (a missing or
/// unparseable attribute means a single cell).
fn span_attr(el: ElementRef<'_>, name: &str) -> usize {
    el.value()
        .attr(name)
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(1)
        .max(1)
}

/// The horizontal alignment a `LaTeXML` cell requests via its `ltx_align_*`
/// class, if any. Vertical (`top`/`middle`/…) and `justify` hints are ignored.
fn cell_align(el: ElementRef<'_>) -> Option<Align> {
    el.value().classes().find_map(|c| match c {
        "ltx_align_left" => Some(Align::Left),
        "ltx_align_center" => Some(Align::Center),
        "ltx_align_right" => Some(Align::Right),
        _ => None,
    })
}

/// Whether a `<tr>` sits inside a table header (`<thead>` or `LaTeXML`'s
/// `ltx_thead`).
fn in_thead(tr: ElementRef<'_>) -> bool {
    tr.ancestors().filter_map(ElementRef::wrap).any(|a| {
        let v = a.value();
        v.name() == "thead" || v.classes().any(|c| c == "ltx_thead")
    })
}

/// Collapse runs of whitespace to single spaces (HTML inline-text semantics).
/// Drops [`SECTION_MARK`] so source text can never forge a section marker.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c == SECTION_MARK {
            continue;
        }
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// A heading's display text for the outline: all descendant text except math
/// (whose `<annotation>` carries raw `TeX` source) and skipped markers.
fn heading_text(el: ElementRef<'_>) -> String {
    fn collect(el: ElementRef<'_>, out: &mut String) {
        for child in el.children() {
            match child.value() {
                Node::Text(t) => out.push_str(&t.text),
                Node::Element(e) if e.name() != "math" => {
                    if let Some(ce) = ElementRef::wrap(child)
                        && !is_skipped(ce)
                    {
                        collect(ce, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut s = String::new();
    collect(el, &mut s);
    collapse_ws(&s).trim().to_string()
}

/// File extension from a URL/path (lowercased, incl. the dot), defaulting to
/// `.png` when none is present.
fn ext_of(src: &str) -> String {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    match path.rsplit('/').next().and_then(|f| f.rsplit_once('.')) {
        Some((_, ext)) if ext.len() <= 5 && ext.chars().all(char::is_alphanumeric) => {
            format!(".{}", ext.to_ascii_lowercase())
        }
        _ => ".png".to_string(),
    }
}

/// Escape a string for a `Typst` double-quoted string literal.
fn typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
    <html><body>
      <nav class="ltx_page_navbar">junk nav</nav>
      <article class="ltx_document">
        <h1 class="ltx_title ltx_title_document">A Tiny Paper</h1>
        <div class="ltx_TOC">Contents... 1 Intro 2</div>
        <section id="S1" class="ltx_section">
          <h2 class="ltx_title ltx_title_section">Introduction</h2>
          <div class="ltx_para"><p class="ltx_p">Hello <em>world</em> with a
            footnote<span class="ltx_note ltx_role_footnote"><sup class="ltx_note_mark">1</sup><span class="ltx_note_outer"><span class="ltx_note_content"><sup class="ltx_note_mark">1</sup><span class="ltx_tag ltx_tag_note">1</span>the note body</span></span></span>
            and math <math><semantics><mfrac><mi>a</mi><mi>b</mi></mfrac><annotation encoding="application/x-tex">\frac{a}{b}</annotation></semantics></math>.</p></div>
        </section>
      </article>
      <footer class="ltx_page_footer">junk footer</footer>
    </body></html>
    "#;

    #[test]
    fn extracts_content_and_drops_chrome() {
        let doc = import(SAMPLE, &NoAssets);
        assert_eq!(doc.title.as_deref(), Some("A Tiny Paper"));
        // chrome and the source TOC are gone
        assert!(!doc.body.contains("junk nav"));
        assert!(!doc.body.contains("junk footer"));
        assert!(!doc.body.contains("Contents..."));
        // The title is extracted, not emitted — front matter belongs to the
        // template, never the body.
        assert!(!doc.body.contains("A Tiny Paper"));
        assert!(!doc.body.contains("#outline()"));
        assert!(doc.body.contains("= Introduction"));
    }

    /// Each heading lands in the outline with `LaTeXML`'s stable section id and
    /// a byte offset pointing at its markup; no marker sentinel survives.
    #[test]
    fn outline_records_sections_with_offsets() {
        let doc = import(SAMPLE, &NoAssets);
        assert_eq!(doc.outline.len(), 1, "body: {}", doc.body);
        let e = &doc.outline[0];
        assert_eq!(
            (e.id.as_str(), e.level, e.title.as_str()),
            ("S1", 1, "Introduction")
        );
        assert!(
            doc.body[e.offset..].starts_with("= Introduction"),
            "offset points at the heading: {}",
            &doc.body[e.offset..]
        );
        assert!(!doc.body.contains('\u{E000}'), "markers stripped");
    }

    #[test]
    fn renders_inline_math_emphasis_and_footnote() {
        let doc = import(SAMPLE, &NoAssets);
        assert!(doc.body.contains("_world_"), "emphasis: {}", doc.body);
        // The note's own mark/tag copies are stripped — Typst's `#footnote`
        // numbers the note itself, so keeping them would triple the number.
        assert!(
            doc.body.contains("#footnote[the note body]"),
            "{}",
            doc.body
        );
        assert!(doc.body.contains('$'), "inline math present: {}", doc.body);
        assert_eq!(doc.stats.math_tex, 1);
        assert_eq!(doc.stats.footnotes, 1);
    }

    /// Real `LaTeXML` list markup carries its own bullet (`ltx_tag_item`); only
    /// the regenerated Typst marker may survive — never `- •`.
    #[test]
    fn list_bullets_are_not_doubled() {
        const LIST: &str = r#"
        <html><body><article class="ltx_document">
          <ul id="S3.I1" class="ltx_itemize">
            <li id="S3.I1.i1" class="ltx_item" style="list-style-type:none;">
              <span class="ltx_tag ltx_tag_item">&#x2022;</span>
              <div class="ltx_para"><p class="ltx_p">Queries come from the decoder.</p></div>
            </li>
            <li id="S3.I1.i2" class="ltx_item" style="list-style-type:none;">
              <span class="ltx_tag ltx_tag_item">&#x2022;</span>
              <div class="ltx_para"><p class="ltx_p">Keys come from the encoder.</p></div>
            </li>
          </ul>
        </article></body></html>
        "#;
        let doc = import(LIST, &NoAssets);
        assert!(
            doc.body.contains("- Queries come from the decoder."),
            "list marker: {}",
            doc.body
        );
        assert!(
            !doc.body.contains('\u{2022}'),
            "source bullet survived: {}",
            doc.body
        );
    }

    /// Display equations arrive as `LaTeXML` layout tables; they must render as
    /// display math with the source's equation number kept right-aligned — never
    /// as a visible Typst table.
    #[test]
    fn equation_tables_render_as_display_math() {
        const EQN: &str = r#"
        <html><body><article class="ltx_document">
          <table id="S3.E1" class="ltx_equation ltx_eqn_table">
            <tbody><tr class="ltx_equation ltx_eqn_row ltx_align_baseline">
              <td class="ltx_eqn_cell ltx_eqn_center_padleft"></td>
              <td class="ltx_eqn_cell ltx_align_center"><math display="block"><semantics><mrow></mrow><annotation encoding="application/x-tex">a+b</annotation></semantics></math></td>
              <td class="ltx_eqn_cell ltx_eqn_center_padright"></td>
              <td class="ltx_eqn_cell ltx_eqn_eqno ltx_align_middle ltx_align_right"><span class="ltx_tag ltx_tag_equation ltx_align_right">(1)</span></td>
            </tr></tbody>
          </table>
        </article></body></html>
        "#;
        let doc = import(EQN, &NoAssets);
        assert!(
            !doc.body.contains("#table("),
            "no visible table: {}",
            doc.body
        );
        assert!(
            doc.body.contains("#grid(columns: (1fr, auto)"),
            "numbered equation grid: {}",
            doc.body
        );
        assert!(
            doc.body.contains("(1)"),
            "equation number kept: {}",
            doc.body
        );
        assert!(doc.body.contains('$'), "display math present: {}", doc.body);
        assert_eq!(doc.stats.math_tex, 1);
    }

    /// An `align` group: each row's math fragments (split around the alignment
    /// point) join into one display equation per row; spacer rows are dropped.
    #[test]
    fn equation_group_rows_each_become_display_math() {
        const GROUP: &str = r#"
        <html><body><article class="ltx_document">
          <table id="Sx1.EGx1" class="ltx_equationgroup ltx_eqn_align ltx_eqn_table">
            <tbody><tr class="ltx_equation ltx_eqn_row">
              <td class="ltx_eqn_cell ltx_eqn_center_padleft"></td>
              <td class="ltx_td ltx_align_right ltx_eqn_cell"><math display="inline"><semantics><annotation encoding="application/x-tex">x</annotation></semantics></math></td>
              <td class="ltx_td ltx_align_left ltx_eqn_cell"><math display="inline"><semantics><annotation encoding="application/x-tex">=a+b</annotation></semantics></math></td>
              <td class="ltx_eqn_cell ltx_eqn_center_padright"></td>
            </tr></tbody>
            <tbody><tr class="ltx_equation ltx_eqn_row">
              <td class="ltx_eqn_cell ltx_eqn_center_padleft"></td>
              <td class="ltx_td ltx_align_right ltx_eqn_cell"><math display="inline"><semantics><annotation encoding="application/x-tex">y</annotation></semantics></math></td>
              <td class="ltx_td ltx_align_left ltx_eqn_cell"><math display="inline"><semantics><annotation encoding="application/x-tex">=c</annotation></semantics></math></td>
              <td class="ltx_eqn_cell ltx_eqn_center_padright"></td>
            </tr></tbody>
          </table>
        </article></body></html>
        "#;
        let doc = import(GROUP, &NoAssets);
        assert!(
            !doc.body.contains("#table("),
            "no visible table: {}",
            doc.body
        );
        assert_eq!(doc.stats.math_tex, 2, "one equation per row: {}", doc.body);
    }

    const SAMPLE_BIB: &str = r##"
    <html><body>
      <article class="ltx_document">
        <p class="ltx_p">As shown<cite class="ltx_cite ltx_citemacro_citep">[<a href="#bib.bib1" class="ltx_ref">1</a>]</cite>
          and also<cite class="ltx_cite"><a href="#S2" class="ltx_ref">Section 2</a></cite>.</p>
        <ol class="ltx_biblist">
          <li id="bib.bib1" class="ltx_bibitem">
            <span class="ltx_tag ltx_role_refnum">[1]</span>
            <span class="ltx_bibblock">Jane Doe. <span class="ltx_text ltx_font_italic">Some Journal</span>, 2020.</span>
          </li>
        </ol>
      </article>
    </body></html>
    "##;

    #[test]
    fn citation_links_to_labelled_bibitem() {
        let doc = import(SAMPLE_BIB, &NoAssets);
        // Citation to a known bibitem links to the sanitized label.
        assert!(
            doc.body.contains("#link(<bib-bib1>)["),
            "cite link: {}",
            doc.body
        );
        // The bibliography entry carries the matching label.
        assert!(doc.body.contains("<bib-bib1>"), "bib label: {}", doc.body);
        // Italic font-span inside the entry is emphasized.
        assert!(
            doc.body.contains("_Some Journal_"),
            "italic span: {}",
            doc.body
        );
        // A non-bib internal anchor stays as plain text (no #link).
        assert!(
            doc.body.contains("Section 2") && !doc.body.contains("#link(<S2>)"),
            "non-bib anchor: {}",
            doc.body
        );
        assert_eq!(doc.stats.citations, 1);
    }

    #[test]
    fn capturing_then_map_resolver_replays_offline() {
        struct OneImage;
        impl AssetResolver for OneImage {
            fn fetch(&self, src: &str) -> Option<Vec<u8>> {
                (src == "fig.png").then(|| b"PNGDATA".to_vec())
            }
        }
        const DOC: &str = r#"<html><body><article class="ltx_document">
            <p class="ltx_p"><img src="fig.png"></p></article></body></html>"#;

        // First pass captures the fetched asset.
        let cap = CapturingResolver::new(&OneImage);
        let first = import(DOC, &cap);
        assert_eq!(first.stats.images_ok, 1);
        let captured = cap.into_assets();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].0, "fig.png");

        // Replaying the captured assets reproduces the import with no live source.
        let replay = import(DOC, &MapResolver::new(captured));
        assert_eq!(replay.stats.images_ok, 1);
        assert_eq!(replay.assets.len(), first.assets.len());
    }

    #[test]
    fn table_emits_alignment_header_and_spans() {
        const TABLE: &str = r#"
        <html><body><article class="ltx_document"><table class="ltx_tabular">
          <thead class="ltx_thead"><tr>
            <th class="ltx_align_left" colspan="2">Wide Head</th>
          </tr></thead>
          <tbody><tr>
            <td class="ltx_align_center">a</td>
            <td class="ltx_align_right">b</td>
          </tr></tbody>
        </table></article></body></html>
        "#;
        let doc = import(TABLE, &NoAssets);
        assert!(doc.body.contains("#table("), "table: {}", doc.body);
        assert!(doc.body.contains("columns: 2"), "columns: {}", doc.body);
        // Per-column alignment from the body cells.
        assert!(
            doc.body.contains("align: (center, right)"),
            "align: {}",
            doc.body
        );
        // Header row wrapped; the wide head spans both columns.
        assert!(doc.body.contains("table.header("), "header: {}", doc.body);
        assert!(
            doc.body.contains("table.cell(colspan: 2, )[Wide Head]"),
            "colspan: {}",
            doc.body
        );
    }
}
