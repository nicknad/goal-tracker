//! Goal versioning through [`history::revise_goal`].

use goal_tracker_core::history;
use goal_tracker_core::model::{Goal, GoalStatus, GoalVersion};
use goal_tracker_core::store::{self, Database};

fn database() -> Database {
    Database::open_in_memory().expect("in-memory database")
}

fn goal() -> Goal {
    Goal {
        id: "goal-1".to_owned(),
        title: "Original".to_owned(),
        description: Some("original description".to_owned()),
        status: GoalStatus::Active,
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        archived_at: None,
    }
}

fn versions(database: &Database) -> Vec<GoalVersion> {
    store::list::<GoalVersion>(database.connection()).expect("list")
}

#[test]
fn revising_a_goal_closes_the_previous_version() {
    let database = database();
    let connection = database.connection();
    let mut goal = goal();
    store::insert(connection, &goal).expect("insert");
    history::revise_goal(connection, &goal).expect("initial version");
    assert_eq!(versions(&database).len(), 1);
    assert!(versions(&database)[0].valid_until.is_none());

    goal.title = "Revised".to_owned();
    history::revise_goal(connection, &goal).expect("revise");

    let versions = versions(&database);
    assert_eq!(versions.len(), 2);
    let open = versions
        .iter()
        .filter(|version| version.valid_until.is_none())
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].title, "Revised");
    let closed = versions
        .iter()
        .find(|version| version.valid_until.is_some())
        .expect("closed version");
    assert_eq!(closed.title, "Original");
}

#[test]
fn unchanged_goals_do_not_create_versions() {
    let database = database();
    let connection = database.connection();
    let goal = goal();
    store::insert(connection, &goal).expect("insert");
    history::revise_goal(connection, &goal).expect("initial");
    history::revise_goal(connection, &goal).expect("unchanged");
    history::revise_goal(connection, &goal).expect("unchanged again");
    assert_eq!(versions(&database).len(), 1);
}

#[test]
fn status_changes_do_not_create_versions() {
    let database = database();
    let connection = database.connection();
    let mut goal = goal();
    store::insert(connection, &goal).expect("insert");
    history::revise_goal(connection, &goal).expect("initial");

    goal.status = GoalStatus::Paused;
    history::revise_goal(connection, &goal).expect("status change");
    assert_eq!(versions(&database).len(), 1);
}
