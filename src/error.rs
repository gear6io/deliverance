use thiserror::Error;

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("channel send failed: {0}")]
    ChannelSend(String),

    #[error("export failed: {0}")]
    ExportFailed(String),

    #[error("max retries exhausted after {attempts} attempt(s)")]
    MaxRetriesExhausted { attempts: usize },

    #[error("arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    Config(String),

    #[error("component error: {0}")]
    Component(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, IngestionError>;
