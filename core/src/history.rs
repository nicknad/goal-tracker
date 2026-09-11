//! Historical records that survive changes to the current hypothesis.
//!
//! Goals are versioned: every change to the title or description of a goal
//! closes the open [`model::GoalVersion`] and opens a new one. The goal's
//! identity (`Goal::id`) is stable across versions. Status and archival are
//! lifecycle, not definition, and do not create a version.

use crate::rusqlite::Connection;
use crate::rusqlite::types::Value;
use crate::{Result, model, store};

/// Closes the open version of `goal` and opens a new one, unless the definition
/// (title and description) is unchanged.
pub fn revise_goal(connection: &Connection, goal: &model::Goal) -> Result<()> {
    let mut versions = store::list_where::<model::GoalVersion>(
        connection,
        &[("goal_id", Value::Text(goal.id.clone()))],
    )?;

    if let Some(current) = versions
        .iter()
        .find(|version| version.valid_until.is_none())
        && current.title == goal.title
        && current.description == goal.description
    {
        return Ok(());
    }

    let timestamp = model::now();
    for version in &mut versions {
        if version.valid_until.is_none() {
            version.valid_until = Some(timestamp.clone());
            store::update(connection, version)?;
        }
    }

    store::insert(
        connection,
        &model::GoalVersion {
            id: model::new_id("goal-version"),
            goal_id: goal.id.clone(),
            title: goal.title.clone(),
            description: goal.description.clone(),
            valid_from: timestamp,
            valid_until: None,
        },
    )
}
