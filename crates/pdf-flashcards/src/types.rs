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
