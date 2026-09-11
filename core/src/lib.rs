//! Domain model and SQLite persistence for goal-tracker.
//!
//! The database stores what happened and what is currently believed. It does not
//! encode a permanent definition of a career.

pub use rusqlite;

pub mod error;
pub mod export;
pub mod history;
pub mod model;
pub mod schema;
pub mod seed;
pub mod store;

pub use error::{Error, Result};
pub use store::Database;
