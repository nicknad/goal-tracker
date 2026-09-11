//! Readable text export of the database.

use std::fmt::Write as _;

use crate::rusqlite::Connection;
use crate::rusqlite::types::Value;
use crate::{Result, model, schema};

/// Renders every table as readable text.
pub fn to_text(connection: &Connection) -> Result<String> {
    let mut output = String::new();
    let _ = writeln!(output, "goal-tracker export {}", model::now());
    for table in schema::TABLES.iter().rev() {
        output.push('\n');
        write_table(connection, table, &mut output)?;
    }
    Ok(output)
}

fn write_table(connection: &Connection, table: &str, output: &mut String) -> Result<()> {
    let mut info = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = info
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let _ = writeln!(output, "== {table} ==");
    let _ = writeln!(output, "{}", columns.join(" | "));
    let _ = writeln!(output, "{}", "-".repeat(columns.len().max(1) * 4));

    let mut select = connection.prepare(&format!("SELECT * FROM {table}"))?;
    let mut rows = select.query([])?;
    while let Some(row) = rows.next()? {
        let mut values = Vec::with_capacity(columns.len());
        for index in 0..columns.len() {
            values.push(format_value(&row.get::<_, Value>(index)?));
        }
        let _ = writeln!(output, "{}", values.join(" | "));
    }
    Ok(())
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Integer(integer) => integer.to_string(),
        Value::Real(real) => real.to_string(),
        Value::Text(text) => text.replace('\r', "").replace('\n', "\\n"),
        Value::Blob(bytes) => format!("<{} bytes>", bytes.len()),
    }
}
