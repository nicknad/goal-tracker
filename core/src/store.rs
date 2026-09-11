//! Generic SQLite persistence built on [`rusqlite`].
//!
//! [`Record`] describes how a type maps to a table. The free functions in this
//! module implement insert, read, update and delete once for every record type.
//! [`Database`] owns the connection, applies schema migrations on open and
//! verifies that the schema matches the model.

use std::path::Path;

use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};

use crate::error::{Error, Result};
use crate::schema;

/// A value that can be stored as a SQLite parameter.
pub trait FieldValue {
    /// Renders the value as an owned SQLite value.
    fn to_value(&self) -> Value;
}

impl FieldValue for String {
    fn to_value(&self) -> Value {
        Value::Text(self.clone())
    }
}

impl FieldValue for Option<String> {
    fn to_value(&self) -> Value {
        self.as_ref()
            .map_or(Value::Null, |value| Value::Text(value.clone()))
    }
}

impl FieldValue for i64 {
    fn to_value(&self) -> Value {
        Value::Integer(*self)
    }
}

/// A row that maps to a single table.
pub trait Record: Sized {
    /// Table name.
    const TABLE: &'static str;
    /// Column names in row order, matching the field order used by `from_row`.
    const COLUMNS: &'static [&'static str];

    /// Named parameters, one per column.
    fn params(&self) -> Vec<(&'static str, Value)>;

    /// Maps a database row to `Self`.
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self>;
}

/// A [`Record`] addressed by a single identifier column.
pub trait Identified: Record {
    /// Identifier value.
    fn id(&self) -> &str;
}

/// Opens a SQLite database, applies migrations and checks the schema.
pub struct Database {
    connection: Connection,
}

impl Database {
    /// Opens or creates the database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an in-memory database.
    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        migrate(&connection)?;
        crate::model::verify_schema(&connection)?;
        Ok(Self { connection })
    }

    /// Borrows the underlying connection.
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Runs `action` inside a transaction, committing on success and rolling
    /// back when it fails.
    pub fn with_transaction<T>(
        &mut self,
        action: impl FnOnce(&Connection) -> Result<T>,
    ) -> Result<T> {
        let transaction = self.connection.transaction()?;
        let value = action(&transaction)?;
        transaction.commit()?;
        Ok(value)
    }
}

/// Applies every migration the database has not seen yet, then records the
/// current version. A version above the list (a database migrated by a
/// pre-reset history) is clamped rather than skipped, so later migrations still
/// apply.
fn migrate(connection: &Connection) -> Result<()> {
    let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    let version = usize::try_from(version).unwrap_or(0);
    for migration in schema::MIGRATIONS.iter().skip(version) {
        connection.execute_batch(migration)?;
    }
    let version = schema::MIGRATIONS.len();
    connection.execute_batch(&format!("PRAGMA user_version = {version}"))?;
    Ok(())
}

fn where_clause(filters: &[(&'static str, Value)]) -> String {
    if filters.is_empty() {
        return String::new();
    }
    let conditions = filters
        .iter()
        .enumerate()
        .map(|(index, (name, _))| format!("{name} = ?{}", index + 1))
        .collect::<Vec<_>>()
        .join(" AND ");
    format!(" WHERE {conditions}")
}

/// Inserts a record.
pub fn insert<R: Record>(connection: &Connection, record: &R) -> Result<()> {
    let params = record.params();
    let table = R::TABLE;
    let columns = params
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ");
    let placeholders = vec!["?"; params.len()].join(", ");
    let sql = format!("INSERT INTO {table} ({columns}) VALUES ({placeholders})");
    connection.execute(
        &sql,
        params_from_iter(params.iter().map(|(_, value)| value.clone())),
    )?;
    Ok(())
}

/// Retrieves a record by identifier.
pub fn get<R: Identified>(connection: &Connection, id: &str) -> Result<Option<R>> {
    Ok(
        list_where::<R>(connection, &[("id", Value::Text(id.to_owned()))])?
            .into_iter()
            .next(),
    )
}

/// Lists every record.
pub fn list<R: Record>(connection: &Connection) -> Result<Vec<R>> {
    list_where::<R>(connection, &[])
}

/// Lists records matching `filters`.
pub fn list_where<R: Record>(
    connection: &Connection,
    filters: &[(&'static str, Value)],
) -> Result<Vec<R>> {
    let table = R::TABLE;
    let columns = R::COLUMNS.join(", ");
    let clause = where_clause(filters);
    let sql = format!("SELECT {columns} FROM {table}{clause}");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params_from_iter(filters.iter().map(|(_, value)| value.clone())),
        R::from_row,
    )?;
    rows.collect::<rusqlite::Result<Vec<R>>>()
        .map_err(Error::from)
}

/// Updates a record by identifier.
pub fn update<R: Identified>(connection: &Connection, record: &R) -> Result<usize> {
    let params = record.params();
    let table = R::TABLE;
    let assignments = params
        .iter()
        .filter(|(name, _)| *name != "id")
        .map(|(name, _)| format!("{name} = ?"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("UPDATE {table} SET {assignments} WHERE id = ?");
    let mut values = params
        .iter()
        .filter(|(name, _)| *name != "id")
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    values.push(Value::Text(record.id().to_owned()));
    connection
        .execute(&sql, params_from_iter(values))
        .map_err(Error::from)
}

/// Deletes a record by identifier.
pub fn delete<R: Identified>(connection: &Connection, id: &str) -> Result<usize> {
    delete_where::<R>(connection, &[("id", Value::Text(id.to_owned()))])
}

/// Deletes every row from every table.
pub fn reset(connection: &Connection) -> Result<()> {
    for table in schema::TABLES {
        connection.execute(&format!("DELETE FROM {table}"), [])?;
    }
    Ok(())
}

/// Deletes records matching `filters`.
pub fn delete_where<R: Record>(
    connection: &Connection,
    filters: &[(&'static str, Value)],
) -> Result<usize> {
    let table = R::TABLE;
    let clause = where_clause(filters);
    let sql = format!("DELETE FROM {table}{clause}");
    connection
        .execute(
            &sql,
            params_from_iter(filters.iter().map(|(_, value)| value.clone())),
        )
        .map_err(Error::from)
}
