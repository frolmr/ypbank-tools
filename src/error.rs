//! Error type shared by the library and its binaries.
//!
//! Every fallible operation in this crate returns [`Result<T>`], which is an
//! alias for `std::result::Result<T, Error>`. The variants distinguish
//! transport failures ([`Error::Io`]) from domain violations
//! ([`Error::Invalid`]) from format-parsing problems with a known line
//! ([`Error::Parse`]). Callers can match on the variant to handle them
//! differently; `Display` is intended for showing to users.

use std::fmt;

/// The single error type returned by this crate.
#[derive(Debug)]
pub enum Error {
    /// An I/O operation failed. Carries the underlying `std::io::Error`,
    /// so callers can inspect `kind()` (e.g. `NotFound`, `ConnectionReset`).
    Io(std::io::Error),

    /// A domain rule was violated while constructing a `Transaction`
    /// (empty category, zero amount, etc.). Not tied to any source location.
    Invalid(String),

    /// A text input could not be parsed at a particular 1-based line.
    /// `msg` describes the specific problem (bad date, unknown kind, …).
    Parse { line: u64, msg: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "io error: {e}"),
            Error::Invalid(msg) => write!(f, "invalid transaction: {msg}"),
            Error::Parse { line, msg } => write!(f, "parse error at line {line}: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Invalid(_) => None,
            Error::Parse { .. } => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
