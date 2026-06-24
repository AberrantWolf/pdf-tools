use thiserror::Error;

#[derive(Error, Debug)]
pub enum FlashcardError {
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("PDF error: {0}")]
    Pdf(String),
    #[error("Config error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Task join error: {0}")]
    TaskJoin(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, FlashcardError>;

#[derive(Debug, Clone, PartialEq)]
pub enum FlashcardWarning {
    /// A CSV row was skipped because it had fewer than 2 columns
    CsvRowSkipped {
        row_number: usize,
        column_count: usize,
    },
    /// The CSV file contained no usable flashcard rows
    EmptyCsv,
    /// The card grid is wider than the printable area; cards will overflow the page.
    GridWiderThanPrintable,
    /// The card grid is taller than the printable area; cards will overflow the page.
    GridTallerThanPrintable,
}

impl std::fmt::Display for FlashcardWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlashcardWarning::CsvRowSkipped {
                row_number,
                column_count,
            } => write!(
                f,
                "Row {row_number}: skipped (has {column_count} column(s), need at least 2)"
            ),
            FlashcardWarning::EmptyCsv => {
                write!(
                    f,
                    "CSV file contained no usable flashcard rows (need at least 2 columns per row)"
                )
            }
            FlashcardWarning::GridWiderThanPrintable => {
                write!(
                    f,
                    "Card grid is wider than the printable area — reduce columns, card width, spacing, or side margins"
                )
            }
            FlashcardWarning::GridTallerThanPrintable => {
                write!(
                    f,
                    "Card grid is taller than the printable area — reduce rows, card height, spacing, or top/bottom margins"
                )
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Flashcard {
    pub front: String,
    pub back: String,
}
