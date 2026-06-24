use crate::options::{Duplex, FlashcardOptions};
use crate::types::{Flashcard, FlashcardError, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pdf_units::{mm_to_pt, pt_to_mm};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

pub async fn generate_pdf(
    cards: &[Flashcard],
    options: &FlashcardOptions,
    output_path: impl AsRef<Path>,
) -> Result<()> {
    let cards = cards.to_vec();
    let options = options.clone();
    let output_path = output_path.as_ref().to_owned();

    let bytes = tokio::task::spawn_blocking(move || generate_flashcard_pdf_bytes(&cards, &options))
        .await??;

    tokio::fs::write(&output_path, bytes).await?;

    Ok(())
}

/// Collect all unique glyphs used across all cards.
/// Returns a map of `glyph_id` -> (char, advance in font units).
fn collect_glyphs(cards: &[Flashcard], face: &ttf_parser::Face<'_>) -> BTreeMap<u16, (char, u16)> {
    let mut glyphs = BTreeMap::new();
    for card in cards {
        for ch in card.front.chars().chain(card.back.chars()) {
            if let Some(glyph_id) = face.glyph_index(ch) {
                glyphs.entry(glyph_id.0).or_insert_with(|| {
                    let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0);
                    (ch, advance)
                });
            }
        }
    }
    glyphs
}

/// Measure text width in points.
fn measure_text_width(text: &str, face: &ttf_parser::Face<'_>, font_size_pt: f32) -> f32 {
    let units_per_em = f32::from(face.units_per_em());
    let mut width = 0.0;
    for ch in text.chars() {
        if let Some(glyph_id) = face.glyph_index(ch) {
            let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0);
            width += (f32::from(advance) / units_per_em) * font_size_pt;
        }
    }
    width
}

/// Encode a text string as hex-encoded glyph IDs for Identity-H CID font.
fn encode_text_hex(text: &str, face: &ttf_parser::Face<'_>) -> String {
    let mut hex = String::with_capacity(text.len() * 4);
    for ch in text.chars() {
        let glyph_id = face.glyph_index(ch).map_or(0, |g| g.0);
        let _ = write!(hex, "{glyph_id:04X}");
    }
    hex
}

/// Build the W (widths) array for CID font from used glyphs.
/// Format: [cid [width] cid [width] ...] for individual entries.
fn build_w_array(glyphs: &BTreeMap<u16, (char, u16)>, units_per_em: u16) -> Object {
    let scale = 1000.0 / f32::from(units_per_em);
    let mut w = Vec::new();
    for (&glyph_id, &(_, advance)) in glyphs {
        w.push(Object::Integer(i64::from(glyph_id)));
        w.push(Object::Array(vec![Object::Integer(
            (f32::from(advance) * scale).round() as i64,
        )]));
    }
    Object::Array(w)
}

/// Build the `ToUnicode` `CMap` stream content.
fn build_tounicode_cmap(glyphs: &BTreeMap<u16, (char, u16)>) -> Vec<u8> {
    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\n");
    cmap.push_str("begincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n");
    cmap.push_str("/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n");
    cmap.push_str("<0000> <FFFF>\n");
    cmap.push_str("endcodespacerange\n");

    // Write bfchar entries in chunks of 100 (PDF spec limit per block)
    let entries: Vec<_> = glyphs.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (glyph_id, (ch, _)) in chunk {
            let unicode = *ch as u32;
            if unicode <= 0xFFFF {
                let _ = writeln!(cmap, "<{glyph_id:04X}> <{unicode:04X}>");
            } else {
                // Surrogate pair for supplementary plane characters
                let hi = ((unicode - 0x10000) >> 10) + 0xD800;
                let lo = ((unicode - 0x10000) & 0x3FF) + 0xDC00;
                let _ = writeln!(cmap, "<{glyph_id:04X}> <{hi:04X}{lo:04X}>");
            }
        }
        cmap.push_str("endbfchar\n");
    }

    cmap.push_str("endcmap\n");
    cmap.push_str("CMapName currentdict /CMap defineresource pop\n");
    cmap.push_str("end end\n");
    cmap.into_bytes()
}

/// Embed a `TrueType` font as a CID-keyed Type0 font in the document.
/// Returns the `ObjectId` of the Type0 font dictionary.
fn embed_font(
    doc: &mut Document,
    font_bytes: &[u8],
    face: &ttf_parser::Face<'_>,
    glyphs: &BTreeMap<u16, (char, u16)>,
) -> ObjectId {
    let units_per_em = face.units_per_em();
    let scale = 1000.0 / f32::from(units_per_em);
    let bbox = face.global_bounding_box();

    // FontFile2: the raw TTF data, compressed
    let mut font_stream = Stream::new(Dictionary::new(), font_bytes.to_vec());
    font_stream
        .dict
        .set("Length1", Object::Integer(font_bytes.len() as i64));
    let _ = font_stream.compress();
    let font_file2_id = doc.add_object(font_stream);

    // FontDescriptor
    let mut fd = Dictionary::new();
    fd.set("Type", Object::Name(b"FontDescriptor".to_vec()));
    fd.set("FontName", Object::Name(b"NotoSansJP-Bold".to_vec()));
    fd.set("Flags", Object::Integer(4)); // Symbolic
    fd.set(
        "FontBBox",
        Object::Array(vec![
            Object::Integer((f32::from(bbox.x_min) * scale).round() as i64),
            Object::Integer((f32::from(bbox.y_min) * scale).round() as i64),
            Object::Integer((f32::from(bbox.x_max) * scale).round() as i64),
            Object::Integer((f32::from(bbox.y_max) * scale).round() as i64),
        ]),
    );
    fd.set("ItalicAngle", Object::Integer(0));
    fd.set(
        "Ascent",
        Object::Integer((f32::from(face.ascender()) * scale).round() as i64),
    );
    fd.set(
        "Descent",
        Object::Integer((f32::from(face.descender()) * scale).round() as i64),
    );
    fd.set(
        "CapHeight",
        Object::Integer(
            face.capital_height()
                .map_or(700, |h| (f32::from(h) * scale).round() as i64),
        ),
    );
    fd.set("StemV", Object::Integer(80));
    fd.set("FontFile2", Object::Reference(font_file2_id));
    let fd_id = doc.add_object(fd);

    // CIDSystemInfo
    let cid_system_info = Dictionary::from_iter(vec![
        (
            "Registry",
            Object::String(b"Adobe".to_vec(), lopdf::StringFormat::Literal),
        ),
        (
            "Ordering",
            Object::String(b"Identity".to_vec(), lopdf::StringFormat::Literal),
        ),
        ("Supplement", Object::Integer(0)),
    ]);

    // CIDFont (DescendantFont)
    let mut cid_font = Dictionary::new();
    cid_font.set("Type", Object::Name(b"Font".to_vec()));
    cid_font.set("Subtype", Object::Name(b"CIDFontType2".to_vec()));
    cid_font.set("BaseFont", Object::Name(b"NotoSansJP-Bold".to_vec()));
    cid_font.set("CIDSystemInfo", Object::Dictionary(cid_system_info));
    cid_font.set("W", build_w_array(glyphs, units_per_em));
    cid_font.set("DW", Object::Integer((1000.0_f32 * scale).round() as i64));
    cid_font.set("FontDescriptor", Object::Reference(fd_id));
    // CIDToGIDMap: Identity mapping (glyph IDs = CIDs)
    cid_font.set("CIDToGIDMap", Object::Name(b"Identity".to_vec()));
    let cid_font_id = doc.add_object(cid_font);

    // ToUnicode CMap
    let cmap_data = build_tounicode_cmap(glyphs);
    let mut cmap_stream = Stream::new(Dictionary::new(), cmap_data);
    let _ = cmap_stream.compress();
    let tounicode_id = doc.add_object(cmap_stream);

    // Type0 font (top-level)
    let mut type0 = Dictionary::new();
    type0.set("Type", Object::Name(b"Font".to_vec()));
    type0.set("Subtype", Object::Name(b"Type0".to_vec()));
    type0.set("BaseFont", Object::Name(b"NotoSansJP-Bold".to_vec()));
    type0.set("Encoding", Object::Name(b"Identity-H".to_vec()));
    type0.set(
        "DescendantFonts",
        Object::Array(vec![Object::Reference(cid_font_id)]),
    );
    type0.set("ToUnicode", Object::Reference(tounicode_id));
    doc.add_object(type0)
}

/// Bottom-left corner (mm, PDF coordinates with y measured from the page bottom)
/// of the front-side cell at grid position `(row, col)`.
fn front_cell_origin(options: &FlashcardOptions, row: usize, col: usize) -> (f32, f32) {
    let x =
        options.margin_left_mm + col as f32 * (options.card_width_mm + options.column_spacing_mm);
    let y = options.page_height_mm
        - options.margin_top_mm
        - (row + 1) as f32 * options.card_height_mm
        - row as f32 * options.row_spacing_mm;
    (x, y)
}

/// Bottom-left corner (mm) of the back-side cell whose front is at `(front_x,
/// front_y)`. The back is the mirror of the front about the page centre, so card
/// backs align behind their fronts once the sheet is flipped — for a long-edge
/// flip that mirrors horizontally, for a short-edge flip vertically. Mirroring
/// the *position* (rather than swapping margins) keeps backs aligned regardless
/// of margins or how much slack the grid leaves on the page.
fn back_cell_origin(options: &FlashcardOptions, front_x: f32, front_y: f32) -> (f32, f32) {
    match options.duplex {
        Duplex::LongEdge => (
            options.page_width_mm - front_x - options.card_width_mm,
            front_y,
        ),
        Duplex::ShortEdge => (
            front_x,
            options.page_height_mm - front_y - options.card_height_mm,
        ),
        // No back is drawn for one-sided output; return the front position so the
        // function is total.
        Duplex::OneSided => (front_x, front_y),
    }
}

// --- Card text layout ------------------------------------------------------
// A card side is the columns assigned to it, joined with newlines. The first
// field is the headword, drawn at the full font size; any further fields are
// detail lines, drawn smaller, wrapped, and clipped to a few lines. The whole
// block is then shrunk if needed so it fits inside the card.

/// Detail fields render at this fraction of the headword's size.
const SECONDARY_SCALE: f32 = 0.6;
/// A detail field wraps to at most this many lines; extras are dropped.
const MAX_SECONDARY_LINES: usize = 3;
/// Line advance as a multiple of the font size (covers ascent + descent + gap).
const LINE_SPACING: f32 = 1.2;
/// Never shrink the headword below this, even if it still overflows.
const MIN_FONT_PT: f32 = 5.0;
/// Inset from the card edges, each side (mm).
const CARD_PADDING_MM: f32 = 2.0;

/// A laid-out line: its text and the font size it's drawn at.
struct LaidLine {
    text: String,
    size_pt: f32,
}

/// Greedy word-wrap `text` to `max_width_mm` at `size_pt`. A single word wider
/// than the limit becomes its own (overflowing) line — the caller's shrink pass
/// brings it back in. Returns no lines for blank input.
fn wrap_field(
    text: &str,
    face: &ttf_parser::Face<'_>,
    size_pt: f32,
    max_width_mm: f32,
) -> Vec<String> {
    let max_width_pt = mm_to_pt(max_width_mm);
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if current.is_empty() || measure_text_width(&candidate, face, size_pt) <= max_width_pt {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Lay out a card side into fitted lines: headword at full size, detail fields
/// smaller/wrapped/clipped, the whole block scaled down until it fits the card.
fn layout_side(
    text: &str,
    face: &ttf_parser::Face<'_>,
    base_size_pt: f32,
    card_width_mm: f32,
    card_height_mm: f32,
) -> Vec<LaidLine> {
    let max_width = (card_width_mm - 2.0 * CARD_PADDING_MM).max(1.0);
    let max_height = (card_height_mm - 2.0 * CARD_PADDING_MM).max(1.0);
    let fields: Vec<&str> = text.split('\n').collect();

    let mut scale = 1.0_f32;
    loop {
        let mut lines: Vec<LaidLine> = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            if field.trim().is_empty() {
                continue;
            }
            let size = if i == 0 {
                base_size_pt * scale
            } else {
                base_size_pt * SECONDARY_SCALE * scale
            };
            let mut wrapped = wrap_field(field, face, size, max_width);
            if i > 0 {
                wrapped.truncate(MAX_SECONDARY_LINES);
            }
            lines.extend(wrapped.into_iter().map(|text| LaidLine {
                text,
                size_pt: size,
            }));
        }

        let total_height: f32 = lines
            .iter()
            .map(|l| pt_to_mm(l.size_pt * LINE_SPACING))
            .sum();
        let fits_width = lines
            .iter()
            .all(|l| pt_to_mm(measure_text_width(&l.text, face, l.size_pt)) <= max_width);

        if (total_height <= max_height && fits_width) || base_size_pt * scale <= MIN_FONT_PT {
            return lines;
        }
        scale *= 0.92;
    }
}

/// Append PDF text operators that draw `text` as a multi-line block, centred in
/// the card cell whose bottom-left corner is `(cell_x, cell_y)` (mm).
fn draw_card_text(
    ops: &mut String,
    text: &str,
    face: &ttf_parser::Face<'_>,
    options: &FlashcardOptions,
    cell_x: f32,
    cell_y: f32,
) {
    let lines = layout_side(
        text,
        face,
        options.font_size_pt,
        options.card_width_mm,
        options.card_height_mm,
    );
    if lines.is_empty() {
        return;
    }

    let center_x = cell_x + options.card_width_mm / 2.0;
    let ascent_ratio = f32::from(face.ascender()) / f32::from(face.units_per_em());
    let total_height: f32 = lines
        .iter()
        .map(|l| pt_to_mm(l.size_pt * LINE_SPACING))
        .sum();

    // Top of the vertically-centred block; walk downward line by line.
    let mut line_top = cell_y + options.card_height_mm.midpoint(total_height);
    for line in &lines {
        let line_height = pt_to_mm(line.size_pt * LINE_SPACING);
        let baseline_y = line_top - pt_to_mm(line.size_pt * ascent_ratio);
        let width_mm = pt_to_mm(measure_text_width(&line.text, face, line.size_pt));
        let x = center_x - width_mm / 2.0;
        let hex = encode_text_hex(&line.text, face);
        let _ = writeln!(
            ops,
            "BT /F1 {} Tf {} {} Td <{hex}> Tj ET",
            line.size_pt,
            mm_to_pt(x),
            mm_to_pt(baseline_y)
        );
        line_top -= line_height;
    }
}

/// Add a single page backed by `content_id` and return its object id.
fn add_page(
    doc: &mut Document,
    pages_tree_id: ObjectId,
    resources_id: ObjectId,
    content_id: ObjectId,
    page_width_pt: f32,
    page_height_pt: f32,
) -> ObjectId {
    let mut page = Dictionary::new();
    page.set("Type", Object::Name(b"Page".to_vec()));
    page.set("Parent", Object::Reference(pages_tree_id));
    page.set(
        "MediaBox",
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Real(page_width_pt),
            Object::Real(page_height_pt),
        ]),
    );
    page.set("Resources", Object::Reference(resources_id));
    page.set("Contents", Object::Reference(content_id));
    doc.add_object(page)
}

fn generate_flashcard_pdf_bytes(
    cards: &[Flashcard],
    options: &FlashcardOptions,
) -> Result<Vec<u8>> {
    let font_bytes: &[u8] = include_bytes!("../fonts/NotoSansJP-Bold.ttf");
    let face = ttf_parser::Face::parse(font_bytes, 0)
        .map_err(|e| FlashcardError::Pdf(format!("Failed to parse font: {e}")))?;

    let mut doc = Document::with_version("1.7");
    let pages_tree_id = doc.new_object_id();

    // Collect all glyphs used across all cards
    let glyphs = collect_glyphs(cards, &face);

    // Embed font
    let font_id = embed_font(&mut doc, font_bytes, &face, &glyphs);

    // Build Resources dictionary (shared across all pages)
    let mut font_dict = Dictionary::new();
    font_dict.set("F1", Object::Reference(font_id));
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(font_dict));
    let resources_id = doc.add_object(resources);

    let cards_per_page = options.rows * options.columns;
    let page_width_pt = mm_to_pt(options.page_width_mm);
    let page_height_pt = mm_to_pt(options.page_height_mm);

    let mut page_refs = Vec::new();

    let double_sided = options.duplex != Duplex::OneSided;

    for chunk in cards.chunks(cards_per_page) {
        let mut front_ops = String::new();
        let mut back_ops = String::new();

        for (i, card) in chunk.iter().enumerate() {
            let row = i / options.columns;
            let col = i % options.columns;

            let (front_x, front_y) = front_cell_origin(options, row, col);
            draw_card_text(
                &mut front_ops,
                &card.front,
                &face,
                options,
                front_x,
                front_y,
            );

            if double_sided {
                let (back_x, back_y) = back_cell_origin(options, front_x, front_y);
                draw_card_text(&mut back_ops, &card.back, &face, options, back_x, back_y);
            }
        }

        let front_content_id =
            doc.add_object(Stream::new(Dictionary::new(), front_ops.into_bytes()));
        let front_page_id = add_page(
            &mut doc,
            pages_tree_id,
            resources_id,
            front_content_id,
            page_width_pt,
            page_height_pt,
        );
        page_refs.push(Object::Reference(front_page_id));

        if double_sided {
            let back_content_id =
                doc.add_object(Stream::new(Dictionary::new(), back_ops.into_bytes()));
            let back_page_id = add_page(
                &mut doc,
                pages_tree_id,
                resources_id,
                back_content_id,
                page_width_pt,
                page_height_pt,
            );
            page_refs.push(Object::Reference(back_page_id));
        }
    }

    // Finalize document structure
    let count = page_refs.len() as i64;
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(page_refs)),
        ("Count", Object::Integer(count)),
    ]);
    doc.objects
        .insert(pages_tree_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_tree_id)),
    ]));
    doc.trailer.set("Root", catalog_id);

    let mut writer = Vec::new();
    doc.save_to(&mut writer)
        .map_err(|e| FlashcardError::Pdf(format!("Failed to save PDF: {e}")))?;

    Ok(writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> FlashcardOptions {
        FlashcardOptions {
            page_width_mm: 200.0,
            page_height_mm: 300.0,
            card_width_mm: 60.0,
            card_height_mm: 90.0,
            ..FlashcardOptions::default()
        }
    }

    #[test]
    fn long_edge_back_mirrors_horizontally() {
        let o = FlashcardOptions {
            duplex: Duplex::LongEdge,
            ..opts()
        };
        let (fx, fy) = front_cell_origin(&o, 0, 0);
        let (bx, by) = back_cell_origin(&o, fx, fy);
        // Back column is the horizontal mirror of the front; rows are unchanged.
        assert!((bx - (o.page_width_mm - fx - o.card_width_mm)).abs() < 1e-3);
        assert!((by - fy).abs() < 1e-3);
    }

    #[test]
    fn short_edge_back_mirrors_vertically() {
        let o = FlashcardOptions {
            duplex: Duplex::ShortEdge,
            ..opts()
        };
        let (fx, fy) = front_cell_origin(&o, 0, 0);
        let (bx, by) = back_cell_origin(&o, fx, fy);
        // Back row is the vertical mirror of the front; columns are unchanged.
        assert!((bx - fx).abs() < 1e-3);
        assert!((by - (o.page_height_mm - fy - o.card_height_mm)).abs() < 1e-3);
    }

    #[test]
    fn back_alignment_survives_a_round_trip_flip() {
        // After physically flipping the sheet, a back cell must land exactly on
        // its front cell. Flipping long-edge negates x about the page centre.
        let o = FlashcardOptions {
            duplex: Duplex::LongEdge,
            ..opts()
        };
        let (fx, fy) = front_cell_origin(&o, 1, 1);
        let (bx, by) = back_cell_origin(&o, fx, fy);
        let flipped_back_x = o.page_width_mm - bx - o.card_width_mm;
        assert!((flipped_back_x - fx).abs() < 1e-3);
        assert!((by - fy).abs() < 1e-3);
    }

    fn test_face() -> ttf_parser::Face<'static> {
        ttf_parser::Face::parse(include_bytes!("../fonts/NotoSansJP-Bold.ttf"), 0).unwrap()
    }

    #[test]
    fn wrap_breaks_long_text_into_multiple_lines() {
        let face = test_face();
        let lines = wrap_field("one two three four five six seven", &face, 12.0, 25.0);
        assert!(lines.len() > 1, "long text should wrap: {lines:?}");
        // No wrapped line is blank, and the words are preserved in order.
        assert_eq!(lines.join(" "), "one two three four five six seven");
    }

    #[test]
    fn headword_is_full_size_and_details_are_smaller() {
        let face = test_face();
        // Field 0 (headword) keeps the base size; field 1 (detail) is scaled down.
        let lines = layout_side("word\ndetail", &face, 24.0, 80.0, 120.0);
        assert_eq!(lines[0].text, "word");
        assert!((lines[0].size_pt - 24.0).abs() < 1e-3);
        assert!(lines[1].size_pt < lines[0].size_pt);
    }

    #[test]
    fn detail_field_is_clipped_to_a_few_lines() {
        let face = test_face();
        // A very wordy detail on a narrow card wraps to many lines, then clips.
        let detail = "alpha bravo charlie delta echo foxtrot golf hotel india juliet";
        let lines = layout_side(&format!("hw\n{detail}"), &face, 12.0, 30.0, 200.0);
        let detail_lines = lines.iter().filter(|l| l.text != "hw").count();
        assert!(
            detail_lines <= MAX_SECONDARY_LINES,
            "detail should clip to {MAX_SECONDARY_LINES}, got {detail_lines}"
        );
    }

    #[test]
    fn block_shrinks_to_fit_a_short_card() {
        let face = test_face();
        // Many lines into a very short card forces the shrink pass below base size.
        let text = "a\nb c d e f g h i j k l m n o p q r s t u v w x y z";
        let lines = layout_side(text, &face, 20.0, 60.0, 12.0);
        assert!(
            lines[0].size_pt < 20.0,
            "headword should shrink to fit, got {}",
            lines[0].size_pt
        );
    }

    #[test]
    fn one_sided_emits_only_front_pages() {
        let o = FlashcardOptions {
            rows: 1,
            columns: 1,
            duplex: Duplex::OneSided,
            ..opts()
        };
        let cards = vec![
            Flashcard {
                front: "a".into(),
                back: "1".into(),
            },
            Flashcard {
                front: "b".into(),
                back: "2".into(),
            },
        ];
        let bytes = generate_flashcard_pdf_bytes(&cards, &o).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        // 2 cards, 1 per sheet, one-sided => 2 pages (no backs).
        assert_eq!(doc.get_pages().len(), 2);
    }

    #[test]
    fn double_sided_emits_front_and_back_pages() {
        let o = FlashcardOptions {
            rows: 1,
            columns: 1,
            duplex: Duplex::LongEdge,
            ..opts()
        };
        let cards = vec![Flashcard {
            front: "a".into(),
            back: "1".into(),
        }];
        let bytes = generate_flashcard_pdf_bytes(&cards, &o).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        // 1 card, double-sided => front + back = 2 pages.
        assert_eq!(doc.get_pages().len(), 2);
    }
}
