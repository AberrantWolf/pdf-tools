use crate::options::FlashcardOptions;
use crate::types::{Flashcard, FlashcardError, Result};
use printpdf::*;
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

fn generate_flashcard_pdf_bytes(
    cards: &[Flashcard],
    options: &FlashcardOptions,
) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::new("Flashcards");

    let font_bytes = include_bytes!("../fonts/NotoSansJP-Bold.ttf");
    let mut font_warnings = Vec::new();
    let font = ParsedFont::from_bytes(font_bytes, 0, &mut font_warnings)
        .ok_or_else(|| FlashcardError::Pdf("Failed to parse font".to_string()))?;
    let font_id = doc.add_font(&font);

    let cards_per_page = options.rows * options.columns;
    let page_width_pt = Mm(options.page_width_mm).into_pt().0;
    let page_height_pt = Mm(options.page_height_mm).into_pt().0;

    for chunk in cards.chunks(cards_per_page) {
        let mut front_ops = Vec::new();
        let mut back_ops = Vec::new();

        for (i, card) in chunk.iter().enumerate() {
            let row = i / options.columns;
            let col = i % options.columns;

            let cell_x_front = options.margin_left_mm
                + col as f32 * (options.card_width_mm + options.column_spacing_mm);
            let cell_y_front = options.page_height_mm
                - options.margin_top_mm
                - (row + 1) as f32 * options.card_height_mm
                - row as f32 * options.row_spacing_mm;

            let center_x_front = cell_x_front + options.card_width_mm / 2.0;
            let y_front =
                cell_y_front + (options.card_height_mm - options.font_size_pt * 25.4 / 72.0) / 2.0;

            let mut text_width = 0.0;
            for ch in card.front.chars() {
                if let Some(glyph_id) = font.lookup_glyph_index(ch as u32) {
                    let advance = font.get_horizontal_advance(glyph_id);
                    text_width += (advance as f32 / 1000.0) * options.font_size_pt;
                }
            }
            let text_width_mm_front = Mm::from(Pt(text_width)).0;
            let x_front = center_x_front - text_width_mm_front / 2.0;

            front_ops.push(Op::StartTextSection);
            front_ops.push(Op::SetFontSize {
                font: font_id.clone(),
                size: Pt(options.font_size_pt),
            });
            front_ops.push(Op::SetTextMatrix {
                matrix: TextMatrix::Translate(Mm(x_front).into_pt(), Mm(y_front).into_pt()),
            });
            front_ops.push(Op::WriteText {
                items: vec![TextItem::Text(card.front.clone())],
                font: font_id.clone(),
            });
            front_ops.push(Op::EndTextSection);

            let mirrored_col = options.columns - 1 - col;
            let cell_x_back = options.margin_right_mm
                + mirrored_col as f32 * (options.card_width_mm + options.column_spacing_mm);
            let cell_y_back = cell_y_front;

            let center_x_back = cell_x_back + options.card_width_mm / 2.0;
            let y_back =
                cell_y_back + (options.card_height_mm - options.font_size_pt * 25.4 / 72.0) / 2.0;

            let mut text_width = 0.0;
            for ch in card.back.chars() {
                if let Some(glyph_id) = font.lookup_glyph_index(ch as u32) {
                    let advance = font.get_horizontal_advance(glyph_id);
                    text_width += (advance as f32 / 1000.0) * options.font_size_pt;
                }
            }

            let text_width_mm_back = Mm::from(Pt(text_width)).0;
            let x_back = center_x_back - text_width_mm_back / 2.0;

            back_ops.push(Op::StartTextSection);
            back_ops.push(Op::SetFontSize {
                font: font_id.clone(),
                size: Pt(options.font_size_pt),
            });
            back_ops.push(Op::SetTextMatrix {
                matrix: TextMatrix::Translate(Mm(x_back).into_pt(), Mm(y_back).into_pt()),
            });
            back_ops.push(Op::WriteText {
                items: vec![TextItem::Text(card.back.clone())],
                font: font_id.clone(),
            });
            back_ops.push(Op::EndTextSection);
        }

        doc.pages.push(PdfPage {
            media_box: Rect {
                x: Pt(0.0),
                y: Pt(0.0),
                width: Pt(page_width_pt),
                height: Pt(page_height_pt),
            },
            trim_box: Rect {
                x: Pt(0.0),
                y: Pt(0.0),
                width: Pt(page_width_pt),
                height: Pt(page_height_pt),
            },
            crop_box: Rect {
                x: Pt(0.0),
                y: Pt(0.0),
                width: Pt(page_width_pt),
                height: Pt(page_height_pt),
            },
            ops: front_ops,
        });

        doc.pages.push(PdfPage {
            media_box: Rect {
                x: Pt(0.0),
                y: Pt(0.0),
                width: Pt(page_width_pt),
                height: Pt(page_height_pt),
            },
            trim_box: Rect {
                x: Pt(0.0),
                y: Pt(0.0),
                width: Pt(page_width_pt),
                height: Pt(page_height_pt),
            },
            crop_box: Rect {
                x: Pt(0.0),
                y: Pt(0.0),
                width: Pt(page_width_pt),
                height: Pt(page_height_pt),
            },
            ops: back_ops,
        });
    }

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    Ok(bytes)
}
