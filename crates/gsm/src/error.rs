#[derive(Debug, thiserror::Error)]
pub enum GsmError {
    #[error("failed to parse CSV: {0}")]
    Csv(#[from] csv::Error),

    #[error("invalid GSM decision '{0}'")]
    InvalidDecision(String),

    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
}
