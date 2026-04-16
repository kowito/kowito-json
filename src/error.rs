use std::fmt;

/// Errors that can occur during kowito-json serialization.
#[derive(Debug)]
pub enum Error {
    /// A custom serialization error produced by `serde::ser::Error::custom`.
    Custom(String),
    /// A parse error with source location.
    Parse { msg: String, line: usize, col: usize },
    /// An I/O error from an underlying `io::Write` target.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Custom(msg) => f.write_str(msg),
            Error::Parse { msg, line, col } => write!(f, "line {line}, col {col}: {msg}"),
            Error::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Custom(_) | Error::Parse { .. } => None,
        }
    }
}

impl Error {
    /// Create a custom error from any displayable message.
    pub fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }

    /// Create a parse error with line and column.
    pub fn parse_at<T: fmt::Display>(msg: T, line: usize, col: usize) -> Self {
        Error::Parse { msg: msg.to_string(), line, col }
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
