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
    /// The CSV/paste contained no data rows
    EmptyCsv,
    /// The card grid is wider than the printable area; cards will overflow the page.
    GridWiderThanPrintable,
    /// The card grid is taller than the printable area; cards will overflow the page.
    GridTallerThanPrintable,
}

impl std::fmt::Display for FlashcardWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlashcardWarning::EmptyCsv => {
                write!(f, "No data rows found")
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
