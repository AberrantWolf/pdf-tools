use printpdf::*;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlashcardError {
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("PDF error: {0}")]
    Pdf(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, FlashcardError>;

#[derive(Debug, Clone)]
pub struct Flashcard {
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone)]
pub struct FlashcardOptions {
    pub page_width_mm: f32,
    pub page_height_mm: f32,
    pub font_size_pt: f32,
    pub cards_per_page: usize,
    pub margin_mm: f32,
}

impl Default for FlashcardOptions {
    fn default() -> Self {
        Self {
            page_width_mm: 215.9,
            page_height_mm: 279.4,
            font_size_pt: 24.0,
            cards_per_page: 4,
            margin_mm: 10.0,
        }
    }
}

pub async fn load_from_csv(path: impl AsRef<Path>) -> Result<Vec<Flashcard>> {
    let path = path.as_ref().to_owned();

    // Read file async
    let contents = tokio::fs::read_to_string(&path).await?;

    // CSV parsing is CPU-bound, spawn blocking
    let cards = tokio::task::spawn_blocking(move || {
        let mut reader = csv::Reader::from_reader(contents.as_bytes());
        let mut cards = Vec::new();

        for result in reader.records() {
            let record = result?;
            if record.len() >= 2 {
                cards.push(Flashcard {
                    front: record[0].to_string(),
                    back: record[1].to_string(),
                });
            }
        }
        Ok::<_, FlashcardError>(cards)
    })
    .await??;

    Ok(cards)
}

pub async fn generate_pdf(
    cards: &[Flashcard],
    options: &FlashcardOptions,
    output_path: impl AsRef<Path>,
) -> Result<()> {
    let cards = cards.to_vec();
    let options = options.clone();
    let output_path = output_path.as_ref().to_owned();

    // PDF generation is CPU-bound, spawn blocking
    let bytes = tokio::task::spawn_blocking(move || generate_pdf_bytes(&cards, &options)).await??;

    // Async file write
    tokio::fs::write(&output_path, bytes).await?;

    Ok(())
}

fn generate_pdf_bytes(cards: &[Flashcard], options: &FlashcardOptions) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::new("Flashcards");

    let cards_per_page = options.cards_per_page;
    let card_height_mm = (options.page_height_mm - 2.0 * options.margin_mm) / cards_per_page as f32;

    let mut pages = Vec::new();

    for chunk in cards.chunks(cards_per_page) {
        let mut front_ops = Vec::new();
        let mut back_ops = Vec::new();

        for (i, card) in chunk.iter().enumerate() {
            let y_front_mm =
                options.page_height_mm - options.margin_mm - (i as f32 + 0.5) * card_height_mm;

            front_ops.push(Op::StartTextSection);
            front_ops.push(Op::SetTextCursor {
                pos: Point {
                    x: Mm(options.margin_mm).into_pt(),
                    y: Mm(y_front_mm).into_pt(),
                },
            });
            front_ops.push(Op::SetFontSizeBuiltinFont {
                font: BuiltinFont::Helvetica,
                size: Pt(options.font_size_pt),
            });
            front_ops.push(Op::WriteTextBuiltinFont {
                items: vec![TextItem::Text(card.front.clone())],
                font: BuiltinFont::Helvetica,
            });
            front_ops.push(Op::EndTextSection);

            let y_back_mm = options.margin_mm + (i as f32 + 0.5) * card_height_mm;

            back_ops.push(Op::StartTextSection);
            back_ops.push(Op::SetTextCursor {
                pos: Point {
                    x: Mm(options.margin_mm).into_pt(),
                    y: Mm(y_back_mm).into_pt(),
                },
            });
            back_ops.push(Op::SetFontSizeBuiltinFont {
                font: BuiltinFont::Helvetica,
                size: Pt(options.font_size_pt),
            });
            back_ops.push(Op::WriteTextBuiltinFont {
                items: vec![TextItem::Text(card.back.clone())],
                font: BuiltinFont::Helvetica,
            });
            back_ops.push(Op::EndTextSection);
        }

        pages.push(PdfPage::new(
            Mm(options.page_width_mm),
            Mm(options.page_height_mm),
            front_ops,
        ));
        pages.push(PdfPage::new(
            Mm(options.page_width_mm),
            Mm(options.page_height_mm),
            back_ops,
        ));
    }

    doc.pages = pages;

    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);

    Ok(bytes)
}
