use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("{0}")]
    Message(String),
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for Error {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

pub(crate) type Result<T> = std::result::Result<T, Error>;
