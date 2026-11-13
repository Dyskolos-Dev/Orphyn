//! Error type shared by every command. It serializes as a plain string, so a rejected `invoke`
//! in the renderer carries a readable message (the same shape as an Electron IPC error).

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The command exists so the bridge is complete, but its module is not written yet.
    #[error("{0}: not implemented")]
    NotImplemented(&'static str),
    #[error("main window not found")]
    NoWindow,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
    #[error("{0}")]
    Other(String),
}

impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_a_message() {
        let json = serde_json::to_string(&Error::NotImplemented("cast_load")).unwrap();
        assert_eq!(json, "\"cast_load: not implemented\"");
    }
}
