//! Error type shared across the core crate.

/// Result alias for the core crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced by the core crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A SQLite error.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// An I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// A stored value could not be parsed into its domain type.
    #[error("invalid {kind} value: {value}")]
    InvalidValue {
        /// Expected domain type.
        kind: &'static str,
        /// Offending value.
        value: String,
    },
    /// A required form field was left empty.
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    /// A form held a different number of values than the entity has fields.
    #[error("form has {got} values, expected {expected}")]
    InvalidForm {
        /// Number of fields the entity declares.
        expected: usize,
        /// Number of values supplied.
        got: usize,
    },
    /// A record addressed by id does not exist.
    #[error("record not found: {0}")]
    NotFound(String),
    /// The database schema does not match the domain model.
    #[error("schema error: {0}")]
    Schema(String),
    /// A seed file could not be read, parsed or validated.
    #[error("seed error: {0}")]
    Seed(String),
}
