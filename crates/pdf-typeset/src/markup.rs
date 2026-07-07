//! Convert source input (Plaintext / Markdown / HTML) into Typst body markup,
//! applying user page-break rules first.

use std::borrow::Cow;
use std::fmt::Write as _;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::asset::AssetRegistry;
use crate::config::{BreakPosition, InputFormat, PageBreakRule, TypesetInput};
use crate::outline::{OutlineEntry, SECTION_MARK, strip_markers};
use crate::typst_table::{Align, Cell, Table as TypstTable};
use crate::{AssetResolver, ImportStats, ImportedDoc, NoAssets};

/// Characters that carry markup meaning in Typst and must be backslash-escaped
/// when emitting literal text. Brackets are included so literal `[`/`]` don't
/// open or close content blocks (notably inside table cells and links).
const INLINE_SPECIALS: &[char] = &[
    '\\', '`', '*', '_', '#', '$', '<', '>', '@', '~', '=', '-', '+', '[', ']',
];

/// Convert an input document to a Typst body, inserting `#pagebreak()` at the
/// boundaries produced by `rules`. `smart` enables Markdown smart punctuation
/// (typographic dashes/ellipses); quote handling is set in the template.
pub fn to_typst_body(input: &TypesetInput, rules: &[PageBreakRule], smart: bool) -> String {
    let pages = paginate(&input.text, rules);
    let chunks: Vec<String> = pages
        .iter()
        .map(|p| convert(p, input.format, smart))
        .collect();
    chunks.join("\n\n#pagebreak()\n\n")
}

fn convert(text: &str, format: InputFormat, smart: bool) -> String {
    match format {
        InputFormat::Markdown => markdown_to_typst(text, smart),
        InputFormat::Plaintext => plaintext_to_typst(text),
        InputFormat::Html => plaintext_to_typst(&html_to_text(text)),
    }
}

// =============================================================================
// Page-break splitting
// =============================================================================

fn line_matches(line: &str, pattern: &str) -> bool {
    let pattern = pattern.trim();
    !pattern.is_empty() && line.trim() == pattern
}

/// Split source text into page chunks at lines matching a rule.
fn paginate(text: &str, rules: &[PageBreakRule]) -> Vec<String> {
    let mut pages: Vec<String> = vec![String::new()];
    for line in text.lines() {
        match rules.iter().find(|r| line_matches(line, &r.pattern)) {
            Some(rule) => match rule.position {
                BreakPosition::Replace => pages.push(String::new()),
                BreakPosition::Before => {
                    pages.push(String::new());
                    push_line(&mut pages, line);
                }
                BreakPosition::After => {
                    push_line(&mut pages, line);
                    pages.push(String::new());
                }
            },
            None => push_line(&mut pages, line),
        }
    }
    let out: Vec<String> = pages.into_iter().filter(|p| !p.trim().is_empty()).collect();
    if out.is_empty() {
        vec![String::new()]
    } else {
        out
    }
}

fn push_line(pages: &mut [String], line: &str) {
    if let Some(last) = pages.last_mut() {
        last.push_str(line);
        last.push('\n');
    }
}

// =============================================================================
// Plaintext → Typst
// =============================================================================

pub(crate) fn escape_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        // Never let source text forge an importer section marker.
        if ch == SECTION_MARK {
            continue;
        }
        if INLINE_SPECIALS.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Drop [`SECTION_MARK`] from text emitted verbatim (code blocks), so raw source
/// can never counterfeit a section marker the way [`escape_inline`] guards
/// escaped text.
fn strip_section_mark(s: &str) -> Cow<'_, str> {
    if s.contains(SECTION_MARK) {
        Cow::Owned(s.chars().filter(|&c| c != SECTION_MARK).collect())
    } else {
        Cow::Borrowed(s)
    }
}

/// Escape one plaintext line: inline specials everywhere, plus a leading numbered
/// enumerator separator (`1.` / `2)`) so it isn't read as a Typst list.
fn escape_plaintext_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let lead = chars.iter().take_while(|c| c.is_whitespace()).count();
    let digits = chars[lead..]
        .iter()
        .take_while(|c| c.is_ascii_digit())
        .count();

    let mut enum_sep: Option<usize> = None;
    if digits > 0 {
        let sep_i = lead + digits;
        if let Some(&sep) = chars.get(sep_i)
            && (sep == '.' || sep == ')')
            && chars.get(sep_i + 1).is_none_or(|c| *c == ' ')
        {
            enum_sep = Some(sep_i);
        }
    }

    let mut out = String::with_capacity(chars.len() + 4);
    for (i, &ch) in chars.iter().enumerate() {
        if INLINE_SPECIALS.contains(&ch) || Some(i) == enum_sep {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn plaintext_to_typst(text: &str) -> String {
    // Escape each line; blank lines survive so Typst sees paragraph breaks.
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(&escape_plaintext_line(line));
        out.push('\n');
    }
    out
}

// =============================================================================
// Markdown → Typst
// =============================================================================

fn heading_level_num(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Streaming state for one Markdown conversion. The editable path uses a
/// non-recording context ([`NoAssets`], no outline); [`import_markdown`] uses a
/// recording one so headings become [`OutlineEntry`]s and images are resolved.
struct MdCtx<'r> {
    assets: AssetRegistry<'r>,
    /// `Some` while recording a document outline: each heading is wrapped in
    /// [`SECTION_MARK`]s and pushed here. `None` for the editable path, which
    /// emits plain headings and no markers (byte-identical to before).
    outline: Option<Vec<OutlineEntry>>,
    /// While a heading is open under a recording context: its entry index and a
    /// plain-text accumulator for the entry title.
    heading: Option<(usize, String)>,
    /// While a resolved image is open: drop its alt-text run.
    in_image: bool,
}

impl<'r> MdCtx<'r> {
    fn new(resolver: &'r dyn AssetResolver, record: bool) -> Self {
        Self {
            assets: AssetRegistry::new(resolver),
            outline: record.then(Vec::new),
            heading: None,
            in_image: false,
        }
    }

    /// Open a heading: plant a section marker and begin recording its title when
    /// this context records an outline, else emit a plain `= ` heading.
    fn start_heading(&mut self, out: &mut String, level: HeadingLevel) {
        let depth = heading_level_num(level);
        out.push('\n');
        if let Some(outline) = &mut self.outline {
            let idx = outline.len();
            outline.push(OutlineEntry {
                id: format!("md-{idx}"),
                level: u8::try_from(depth).unwrap_or(u8::MAX),
                title: String::new(),
                offset: 0, // filled in by strip_markers
            });
            out.push(SECTION_MARK);
            let _ = write!(out, "{idx}");
            out.push(SECTION_MARK);
            self.heading = Some((idx, String::new()));
        }
        for _ in 0..depth {
            out.push('=');
        }
        out.push(' ');
    }

    /// Close a heading: store the accumulated plain text on its outline entry.
    fn end_heading(&mut self, out: &mut String) {
        if let Some((idx, title)) = self.heading.take()
            && let Some(outline) = self.outline.as_mut()
            && let Some(entry) = outline.get_mut(idx)
        {
            entry.title = title.trim().to_string();
        }
        out.push_str("\n\n");
    }

    /// Record heading title text while a heading is open (ignored otherwise).
    fn note_heading_text(&mut self, text: &str) {
        if let Some((_, title)) = &mut self.heading {
            title.push_str(text);
        }
    }
}

/// Drive the Markdown parser into Typst markup through `ctx`. Shared by the
/// editable text path (via [`markdown_to_typst`]) and [`import_markdown`].
fn markdown_events(md: &str, smart: bool, ctx: &mut MdCtx) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    if smart {
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    let parser = Parser::new_ext(md, opts);

    let mut out = String::new();
    // For each open list: the running ordered index, or None for a bullet list.
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    // Set while accumulating a Markdown table; cells contain only inline content.
    let mut table: Option<TableBuilder> = None;

    for event in parser {
        // Inside a table, inline content is routed into the current cell and the
        // structural events build up the grid; everything else is suppressed.
        if let Some(tb) = table.as_mut() {
            if tb.handle(&event) {
                continue;
            }
            // End(Table) falls through here so we can flush and clear the state.
            if matches!(event, Event::End(TagEnd::Table)) {
                out.push_str(&table.take().expect("in table").render());
                continue;
            }
        }

        // Drop the alt-text run of a successfully-resolved image.
        if ctx.in_image {
            if matches!(event, Event::End(TagEnd::Image)) {
                ctx.in_image = false;
            }
            continue;
        }

        match event {
            Event::Start(Tag::Table(aligns)) => table = Some(TableBuilder::new(aligns)),
            Event::Start(Tag::Heading { level, .. }) => ctx.start_heading(&mut out, level),
            Event::End(TagEnd::Heading(_)) => ctx.end_heading(&mut out),
            Event::End(TagEnd::Paragraph) => out.push_str("\n\n"),

            Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => out.push('_'),
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => out.push('*'),
            Event::Start(Tag::Strikethrough) => out.push_str("#strike["),

            Event::Start(Tag::BlockQuote(_)) => out.push_str("#quote(block: true)[\n"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("]\n\n"),

            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                out.push_str("```");
                out.push_str(&lang);
                out.push('\n');
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                out.push_str("```\n\n");
            }

            Event::Start(Tag::List(start)) => list_stack.push(start),
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                if list_stack.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                let depth = list_stack.len().saturating_sub(1);
                out.push('\n');
                for _ in 0..depth {
                    out.push_str("  ");
                }
                match list_stack.last_mut() {
                    Some(Some(n)) => {
                        let _ = write!(out, "{n}. ");
                        *n += 1;
                    }
                    _ => out.push_str("- "),
                }
            }

            // A resolvable image becomes a Typst `image()`; its alt-text run is
            // then dropped. An unresolvable one falls through to its alt text (the
            // only behavior available on the editable, asset-less path).
            Event::Start(Tag::Image { dest_url, .. }) => {
                if let Some(name) = ctx.assets.image(&dest_url) {
                    let _ = write!(out, "#box(image(\"{name}\"))");
                    ctx.in_image = true;
                }
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                let _ = write!(out, "#link(\"{}\")[", escape_url(&dest_url));
            }
            // Close both inline wrappers (strikethrough and links) with `]`.
            Event::End(TagEnd::Strikethrough | TagEnd::Link) => out.push(']'),

            Event::Text(t) => {
                let t = strip_section_mark(&t);
                if in_code_block {
                    out.push_str(&t);
                } else {
                    out.push_str(&escape_inline(&t));
                }
                ctx.note_heading_text(&t);
            }
            Event::Code(t) => {
                out.push('`');
                out.push_str(&t);
                out.push('`');
                ctx.note_heading_text(&t);
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str(" \\\n"),
            Event::Rule => out.push_str("\n#line(length: 100%)\n\n"),

            _ => {}
        }
    }
    out
}

/// Convert Markdown for the editable text path: no assets, no recorded outline —
/// byte-identical to the pre-import behavior.
fn markdown_to_typst(md: &str, smart: bool) -> String {
    let mut ctx = MdCtx::new(&NoAssets, false);
    markdown_events(md, smart, &mut ctx)
}

/// Convert a Markdown document into a typesettable [`ImportedDoc`] — the Markdown
/// counterpart to [`crate::import_html`]. Images are fetched through `resolver`
/// (relative paths resolve against the source file's directory when it is a
/// local-file resolver), and every heading becomes an [`OutlineEntry`] driving
/// per-section overrides. Front matter (title page, table of contents) stays
/// template-owned, so no title is extracted from the body.
pub fn import_markdown(md: &str, resolver: &dyn AssetResolver, smart: bool) -> ImportedDoc {
    let mut ctx = MdCtx::new(resolver, true);
    let raw = markdown_events(md, smart, &mut ctx);
    let mut outline = ctx.outline.take().unwrap_or_default();
    let mut body = strip_markers(&raw, &mut outline);

    // A document that opens with a heading carries a leading newline, which would
    // push the first heading past offset 0 and fake a "front matter" section.
    // Trim it and shift the recorded offsets to match.
    let shift = body.len() - body.trim_start().len();
    if shift > 0 {
        body = body[shift..].to_string();
        for entry in &mut outline {
            entry.offset = entry.offset.saturating_sub(shift);
        }
    }

    let stats = ImportStats {
        images_ok: ctx.assets.ok,
        images_failed: ctx.assets.failed,
        ..ImportStats::default()
    };
    log::info!(
        "imported markdown: images {} ok / {} failed, {} headings",
        stats.images_ok,
        stats.images_failed,
        outline.len()
    );
    ImportedDoc {
        body,
        assets: ctx.assets.into_assets(),
        outline,
        title: None,
        stats,
    }
}

fn escape_url(url: &str) -> String {
    url.replace('\\', "\\\\").replace('"', "\\\"")
}

// =============================================================================
// Markdown tables → Typst `#table`
// =============================================================================

/// Accumulates a Markdown table's cells, then renders a Typst `#table`. Cell
/// content is inline-only Typst markup; visual styling (borders, header shading,
/// zebra striping) is applied globally by the template's `#set table` rules.
struct TableBuilder {
    aligns: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    cur_row: Vec<String>,
    cur_cell: String,
    /// Number of leading rows that form the header (wrapped in `table.header`).
    header_rows: usize,
}

impl TableBuilder {
    fn new(aligns: Vec<Alignment>) -> Self {
        Self {
            aligns,
            rows: Vec::new(),
            cur_row: Vec::new(),
            cur_cell: String::new(),
            header_rows: 0,
        }
    }

    /// Feed one parser event, routing inline content into the current cell and
    /// building up the grid from the structural events. Returns `true` if the
    /// event was consumed; only `End(Table)` returns `false`, signalling the
    /// caller to flush and render. Structural row/head starts and any stray
    /// events fall through the catch-all (consumed, no effect).
    fn handle(&mut self, event: &Event) -> bool {
        match event {
            Event::End(TagEnd::Table) => return false, // caller flushes and renders
            Event::End(TagEnd::TableHead) => {
                self.finish_row();
                self.header_rows = self.rows.len();
            }
            Event::End(TagEnd::TableRow) => self.finish_row(),
            Event::Start(Tag::TableCell) => self.cur_cell.clear(),
            Event::End(TagEnd::TableCell) => self.finish_cell(),

            // Inline content routed into the current cell.
            Event::Text(t) => self.cur_cell.push_str(&escape_inline(t)),
            Event::Code(t) => {
                self.cur_cell.push('`');
                self.cur_cell.push_str(t);
                self.cur_cell.push('`');
            }
            Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => self.cur_cell.push('_'),
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => self.cur_cell.push('*'),
            Event::Start(Tag::Strikethrough) => self.cur_cell.push_str("#strike["),
            Event::End(TagEnd::Strikethrough | TagEnd::Link) => self.cur_cell.push(']'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let _ = write!(self.cur_cell, "#link(\"{}\")[", escape_url(dest_url));
            }
            Event::SoftBreak | Event::HardBreak => self.cur_cell.push(' '),
            _ => {}
        }
        true
    }

    fn finish_cell(&mut self) {
        let cell = std::mem::take(&mut self.cur_cell);
        self.cur_row.push(cell.trim().to_string());
    }

    fn finish_row(&mut self) {
        if !self.cur_row.is_empty() {
            self.rows.push(std::mem::take(&mut self.cur_row));
        }
    }

    fn render(&self) -> String {
        let columns = self
            .aligns
            .len()
            .max(self.rows.iter().map(Vec::len).max().unwrap_or(0))
            .max(1);
        TypstTable {
            columns,
            aligns: self.aligns.iter().map(|a| align_to_typst(*a)).collect(),
            header_rows: self.header_rows,
            // Markdown cells never span; map each to a plain 1×1 cell.
            rows: self
                .rows
                .iter()
                .map(|row| row.iter().map(|c| Cell::new(c.as_str())).collect())
                .collect(),
        }
        .render()
    }
}

fn align_to_typst(a: Alignment) -> Align {
    match a {
        Alignment::Left | Alignment::None => Align::Left,
        Alignment::Center => Align::Center,
        Alignment::Right => Align::Right,
    }
}

// =============================================================================
// HTML → text (basic; structured HTML support is a follow-up)
// =============================================================================

/// Strip HTML tags to readable text, turning block-level tags into blank lines
/// and decoding common entities. Good enough for simple HTML; rich structure
/// (bold/headings) is a planned follow-up using a real HTML parser.
fn html_to_text(html: &str) -> String {
    const BLOCK_TAGS: &[&str] = &[
        "p",
        "div",
        "br",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "li",
        "ul",
        "ol",
        "blockquote",
        "section",
        "article",
        "header",
        "footer",
        "pre",
        "table",
        "tr",
    ];

    let mut out = String::new();
    let mut chars = html.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            // Consume the tag.
            let mut tag = String::new();
            for c in chars.by_ref() {
                if c == '>' {
                    break;
                }
                tag.push(c);
            }
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase();
            if BLOCK_TAGS.contains(&name.as_str()) {
                out.push_str("\n\n");
            }
        } else {
            out.push(ch);
        }
    }
    decode_entities(&out)
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_table_becomes_typst_table() {
        let md = "\
| Name | Qty |
|:-----|----:|
| Ink  | 3   |
| Paper| 12  |
";
        let out = markdown_to_typst(md, false);
        assert!(out.contains("#table("), "expected a #table call:\n{out}");
        assert!(out.contains("columns: 2"), "two columns expected:\n{out}");
        // Left/right alignment carried from the `:---`/`---:` delimiter row.
        assert!(out.contains("align: (left, right)"), "alignment:\n{out}");
        // Header row is wrapped so the template can style it.
        assert!(out.contains("table.header("), "header wrapper:\n{out}");
        assert!(
            out.contains("[Name]") && out.contains("[Paper]"),
            "cells:\n{out}"
        );
    }

    #[test]
    fn table_cell_brackets_are_escaped() {
        let md = "| A |\n|---|\n| x[y] |\n";
        let out = markdown_to_typst(md, false);
        assert!(out.contains("x\\[y\\]"), "brackets must be escaped:\n{out}");
    }

    #[test]
    fn smart_punctuation_toggles_dashes() {
        let plain = markdown_to_typst("a -- b", false);
        let smart = markdown_to_typst("a -- b", true);
        assert_ne!(plain, smart, "smart punctuation should change the output");
    }

    /// A one-image resolver, like the HTML importer's fixtures.
    struct OneImage;
    impl AssetResolver for OneImage {
        fn fetch(&self, src: &str) -> Option<Vec<u8>> {
            (src == "fig.png").then(|| b"PNGDATA".to_vec())
        }
    }

    #[test]
    fn import_markdown_resolves_relative_image_and_drops_alt() {
        let doc = import_markdown("![a caption](fig.png)\n", &OneImage, false);
        assert_eq!(doc.stats.images_ok, 1);
        assert!(
            doc.body.contains("#box(image(\"img-"),
            "image emitted: {}",
            doc.body
        );
        // The alt text is replaced by the image, not kept alongside it.
        assert!(!doc.body.contains("a caption"), "alt dropped: {}", doc.body);
        assert_eq!(doc.assets.len(), 1);
    }

    #[test]
    fn import_markdown_unresolved_image_keeps_alt() {
        let doc = import_markdown("![missing fig](none.png)\n", &NoAssets, false);
        assert_eq!(doc.stats.images_failed, 1);
        assert_eq!(doc.stats.images_ok, 0);
        assert!(doc.body.contains("missing fig"), "alt kept: {}", doc.body);
    }

    /// Headings become outline entries whose offsets point at their `=` markup,
    /// no title is extracted (front matter stays template-owned), and a
    /// heading-first document has no phantom leading front matter.
    #[test]
    fn import_markdown_records_outline_offsets() {
        let doc = import_markdown("# Intro\n\ntext\n\n## Details\n\nmore\n", &NoAssets, false);
        assert!(doc.title.is_none(), "no title extracted");
        assert_eq!(doc.outline.len(), 2);
        assert_eq!(
            (doc.outline[0].level, doc.outline[0].title.as_str()),
            (1, "Intro")
        );
        assert_eq!(
            (doc.outline[1].level, doc.outline[1].title.as_str()),
            (2, "Details")
        );
        assert_eq!(doc.outline[0].offset, 0, "no phantom front matter");
        assert!(doc.body[doc.outline[0].offset..].starts_with("= Intro"));
        assert!(doc.body[doc.outline[1].offset..].starts_with("== Details"));
        assert!(!doc.body.contains('\u{E000}'), "markers stripped");
    }

    #[test]
    fn import_markdown_heading_title_ignores_inline_markup() {
        let doc = import_markdown("## The *fast* `path`\n", &NoAssets, false);
        assert_eq!(doc.outline.len(), 1);
        assert_eq!(doc.outline[0].title, "The fast path");
        // The body heading keeps the emphasis/raw markup.
        assert!(
            doc.body.contains("== The _fast_ `path`"),
            "body markup: {}",
            doc.body
        );
    }

    /// Front matter before the first heading is preserved with a non-zero first
    /// offset, so the rail can offer a "front matter" row.
    #[test]
    fn import_markdown_keeps_front_matter_before_first_heading() {
        let doc = import_markdown("A preamble paragraph.\n\n# One\n", &NoAssets, false);
        assert_eq!(doc.outline.len(), 1);
        assert!(doc.outline[0].offset > 0, "front matter precedes heading");
        assert!(doc.body.starts_with("A preamble paragraph."));
    }
}
