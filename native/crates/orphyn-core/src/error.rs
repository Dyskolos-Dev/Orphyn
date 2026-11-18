//! Error type of the core crate (the variants of `src-tauri/src/error.rs` minus the Tauri ones).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("no data directory on this system")]
    NoDataDir,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
