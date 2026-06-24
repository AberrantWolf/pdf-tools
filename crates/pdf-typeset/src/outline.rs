//! Document outline for imported documents, and per-section overrides.
//!
//! The HTML importer records one [`OutlineEntry`] per heading it emits, carrying
//! a byte offset into the converted body. A section's extent runs from its
//! heading to the next heading of equal or shallower level, so hiding an entry
//! hides its whole subtree. [`assemble_body`] applies a set of
//! [`SectionOverride`]s to the cached body — a cheap string pass, so the
//! expensive DOM walk/conversion never re-runs when the user toggles sections.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Override key for the content before the first heading (authors, abstract
/// preamble, …). It has no [`OutlineEntry`]; only `hidden` applies.
pub const FRONT_MATTER_ID: &str = "front-matter";

/// Sentinel the importer plants at each heading so offsets can be recovered
/// after the recursive render (private-use char; stripped from source text).
pub(crate) const SECTION_MARK: char = '\u{E000}';

/// Remove the importer's section markers (`SECTION_MARK` + entry index +
/// `SECTION_MARK`) from `raw`, recording each marker's position in the cleaned
/// string as the matching entry's offset.
pub(crate) fn strip_markers(raw: &str, outline: &mut [OutlineEntry]) -> String {
    let mut clean = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find(SECTION_MARK) {
        clean.push_str(&rest[..start]);
        rest = &rest[start + SECTION_MARK.len_utf8()..];
        // The guards can't fail for importer-emitted markers (and the importer
        // strips the marker char from source text); on a malformed marker the
        // stray char is simply dropped.
        if let Some(end) = rest.find(SECTION_MARK)
            && let Ok(idx) = rest[..end].parse::<usize>()
            && let Some(entry) = outline.get_mut(idx)
        {
            entry.offset = clean.len();
            rest = &rest[end + SECTION_MARK.len_utf8()..];
        }
    }
    clean.push_str(rest);
    clean
}

/// One heading in an imported document's outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineEntry {
    /// Stable section id — `LaTeXML`'s (e.g. `S3.SS2`, `abstract1`) when the
    /// markup carries one, else a synthesized index. Keys [`SectionOverride`]s,
    /// so saved overrides survive a re-conversion of the same document.
    pub id: String,
    /// Typst heading level (1-based).
    pub level: u8,
    /// Plain-text heading title, for display.
    pub title: String,
    /// Byte offset of the heading's markup in the body.
    pub offset: usize,
}

/// Per-section presentation overrides, keyed by [`OutlineEntry::id`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SectionOverride {
    /// Drop the section (heading + content + subsections) from the output.
    pub hidden: bool,
    /// Start the section on a new page (`pagebreak(weak: true)`).
    pub break_before: bool,
}

impl SectionOverride {
    /// Whether this override changes anything — defaults need not be stored.
    pub fn is_default(self) -> bool {
        self == Self::default()
    }
}

/// Apply `overrides` to a converted `body`: drop hidden sections (each with its
/// subtree) and prepend a weak page break to forced ones. Citations that link to
/// labels defined inside hidden sections (e.g. bibliography entries) are
/// unwrapped to their plain text — a `#link` to a missing label is a Typst
/// compile error. Borrows the body unchanged when no override has any effect.
pub fn assemble_body<'a, S: std::hash::BuildHasher>(
    body: &'a str,
    outline: &[OutlineEntry],
    overrides: &HashMap<String, SectionOverride, S>,
) -> Cow<'a, str> {
    let effective = |id: &str| overrides.get(id).copied().unwrap_or_default();
    let front_hidden = effective(FRONT_MATTER_ID).hidden;
    if !front_hidden && outline.iter().all(|e| effective(&e.id).is_default()) {
        return Cow::Borrowed(body);
    }

    let mut out = String::with_capacity(body.len());
    let mut hidden_text = String::new();
    let mut cursor = if front_hidden {
        let end = outline.first().map_or(body.len(), |e| e.offset);
        hidden_text.push_str(&body[..end]);
        end
    } else {
        0
    };

    for (i, entry) in outline.iter().enumerate() {
        if entry.offset < cursor {
            continue; // swallowed by a hidden enclosing section
        }
        out.push_str(&body[cursor..entry.offset]);
        cursor = entry.offset;
        let ov = effective(&entry.id);
        if ov.hidden {
            let end = section_end(outline, i, body.len());
            hidden_text.push_str(&body[cursor..end]);
            cursor = end;
        } else if ov.break_before {
            out.push_str("#pagebreak(weak: true)\n");
        }
    }
    out.push_str(&body[cursor..]);

    let dropped = labels_in(&hidden_text);
    if !dropped.is_empty() {
        unwrap_links_to(&mut out, &dropped);
    }
    Cow::Owned(out)
}

/// Where section `i` ends: at the next heading of equal or shallower level, or
/// the end of the body.
fn section_end(outline: &[OutlineEntry], i: usize, body_len: usize) -> usize {
    let level = outline[i].level;
    outline[i + 1..]
        .iter()
        .find(|e| e.level <= level)
        .map_or(body_len, |e| e.offset)
}

/// Typst label definitions (`<name>`) occurring in `text`. Matches the labels
/// the importer emits on bibliography entries; an occasional false positive is
/// harmless — it only matters if a visible `#link` targets it, and links are
/// only emitted to labels the importer defined.
fn labels_in(text: &str) -> HashSet<String> {
    let mut labels = HashSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('>')
            && end > 0
            && rest[..end]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            labels.insert(rest[..end].to_string());
            rest = &rest[end + 1..];
        }
    }
    labels
}

/// Replace every `#link(<label>)[content]` whose label is in `labels` with its
/// plain `content`. Bracket nesting inside the content is respected.
fn unwrap_links_to(out: &mut String, labels: &HashSet<String>) {
    for label in labels {
        let needle = format!("#link(<{label}>)[");
        while let Some(start) = out.find(&needle) {
            let content_start = start + needle.len();
            let Some(content_len) = matched_bracket_len(&out[content_start..]) else {
                break; // malformed markup; leave it rather than loop forever
            };
            let content = out[content_start..content_start + content_len].to_string();
            out.replace_range(start..=(content_start + content_len), &content);
        }
    }
}

/// Length of the content before the `]` matching an already-consumed `[`.
fn matched_bracket_len(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, level: u8, offset: usize) -> OutlineEntry {
        OutlineEntry {
            id: id.to_string(),
            level,
            title: id.to_string(),
            offset,
        }
    }

    fn hidden() -> SectionOverride {
        SectionOverride {
            hidden: true,
            break_before: false,
        }
    }

    /// `front [S1 intro [S1.1 sub]] [S2 methods]` with offsets at each bracket.
    const BODY: &str = "front\n= Intro\nintro text\n== Sub\nsub text\n= Methods\nmethods text\n";

    fn outline() -> Vec<OutlineEntry> {
        vec![
            entry("S1", 1, BODY.find("= Intro").unwrap()),
            entry("S1.1", 2, BODY.find("== Sub").unwrap()),
            entry("S2", 1, BODY.find("= Methods").unwrap()),
        ]
    }

    #[test]
    fn no_overrides_borrows_unchanged() {
        let out = assemble_body(BODY, &outline(), &HashMap::new());
        assert!(matches!(out, Cow::Borrowed(_)));
        assert_eq!(out, BODY);
    }

    #[test]
    fn hiding_a_section_hides_its_subtree_only() {
        let overrides = HashMap::from([("S1".to_string(), hidden())]);
        let out = assemble_body(BODY, &outline(), &overrides);
        assert!(!out.contains("Intro") && !out.contains("Sub"), "{out}");
        assert!(
            out.starts_with("front") && out.contains("= Methods"),
            "{out}"
        );
    }

    #[test]
    fn hiding_a_subsection_keeps_its_parent_and_siblings() {
        let overrides = HashMap::from([("S1.1".to_string(), hidden())]);
        let out = assemble_body(BODY, &outline(), &overrides);
        assert!(
            out.contains("= Intro") && out.contains("intro text"),
            "{out}"
        );
        assert!(!out.contains("== Sub"), "{out}");
        assert!(out.contains("= Methods"), "{out}");
    }

    #[test]
    fn break_before_prepends_a_weak_pagebreak() {
        let overrides = HashMap::from([(
            "S2".to_string(),
            SectionOverride {
                hidden: false,
                break_before: true,
            },
        )]);
        let out = assemble_body(BODY, &outline(), &overrides);
        assert!(
            out.contains("#pagebreak(weak: true)\n= Methods"),
            "break lands immediately before the heading: {out}"
        );
    }

    #[test]
    fn front_matter_can_be_hidden() {
        let overrides = HashMap::from([(FRONT_MATTER_ID.to_string(), hidden())]);
        let out = assemble_body(BODY, &outline(), &overrides);
        assert!(!out.contains("front"), "{out}");
        assert!(out.starts_with("= Intro"), "{out}");
    }

    #[test]
    fn citations_into_hidden_sections_unwrap_to_plain_text() {
        let body = "See #link(<bib-bib1>)[Doe 2020] here.\n= Refs\nentry <bib-bib1>\n";
        let outline = vec![entry("refs", 1, body.find("= Refs").unwrap())];
        let overrides = HashMap::from([("refs".to_string(), hidden())]);
        let out = assemble_body(body, &outline, &overrides);
        assert!(
            out.contains("See Doe 2020 here.") && !out.contains("#link"),
            "link to a dropped label is unwrapped: {out}"
        );
    }

    #[test]
    fn citations_to_visible_labels_are_kept() {
        let body =
            "See #link(<bib-bib1>)[Doe] here.\n= Hide me\nnothing\n= Refs\nentry <bib-bib1>\n";
        let outline = vec![
            entry("gone", 1, body.find("= Hide me").unwrap()),
            entry("refs", 1, body.find("= Refs").unwrap()),
        ];
        let overrides = HashMap::from([("gone".to_string(), hidden())]);
        let out = assemble_body(body, &outline, &overrides);
        assert!(out.contains("#link(<bib-bib1>)[Doe]"), "{out}");
        assert!(out.contains("<bib-bib1>"), "{out}");
    }
}
