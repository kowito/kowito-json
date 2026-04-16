use std::fmt;

/// Errors that can occur during kowito-json serialization.
#[derive(Debug)]
pub enum Error {
    /// A custom serialization error produced by `serde::ser::Error::custom`.
    Custom(String),
    /// An I/O error from an underlying `io::Write` target.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Custom(msg) => f.write_str(msg),
            Error::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Custom(_) => None,
        }
    }
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

/// Convenience `Result` alias for kowito-json operations.
pub type Result<T> = std::result::Result<T, Error>;
