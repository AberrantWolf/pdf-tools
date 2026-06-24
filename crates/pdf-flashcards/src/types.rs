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
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Flashcard {
    pub front: String,
    pub back: String,
}
