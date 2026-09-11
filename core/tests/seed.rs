//! Seed loading, validation and idempotency.

use std::io::Write;

use goal_tracker_core::store::Database;
use goal_tracker_core::{model, seed, store};
use tempfile::NamedTempFile;

const VALID: &str = r#"
[goal]
id = "goal-1"
title = "Grow"
description = "description"
status = "active"

[decision]
id = "decision-1"
description = "Decision"
context = "Context"
decision = "Decision text"
rationale = "Rationale"

[[skills]]
id = "skill-1"
name = "Rust"
description = "Systems"
status = "learning"
level = 7

[[skills]]
id = "skill-2"
name = "FFI"
description = "Boundaries"
status = "active"

[[milestones]]
id = "milestone-1"
title = "Ship"
status = "planned"
criteria = ["Works", "Tested"]
evidence = "Repository"
validation = "Review"
skills = ["skill-1"]
projects = ["project-1"]
depends_on = []

[[projects]]
id = "project-1"
name = "Project"
description = "Description"
status = "active"

[[skill_relationships]]
from = "skill-1"
to = "skill-2"
relationship = "related"
"#;

fn seed_file(contents: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp seed file");
    file.write_all(contents.as_bytes())
        .expect("write temp seed");
    file
}

fn database() -> Database {
    Database::open_in_memory().expect("in-memory database")
}

#[test]
fn populate_inserts_once_and_is_idempotent() {
    let seed = seed_file(VALID);
    let mut database = database();

    let report = seed::populate(&mut database, seed.path(), false)
        .expect("seed")
        .expect("first seed inserts");
    assert_eq!(report.goals, 1);
    assert_eq!(report.skills, 2);
    assert_eq!(report.milestones, 1);
    assert_eq!(report.projects, 1);

    assert!(
        seed::populate(&mut database, seed.path(), false)
            .expect("seed")
            .is_none()
    );
}

#[test]
fn force_resets_and_reseeds() {
    let seed = seed_file(VALID);
    let mut database = database();
    seed::populate(&mut database, seed.path(), false)
        .expect("seed")
        .expect("first seed");
    seed::populate(&mut database, seed.path(), true)
        .expect("seed")
        .expect("force seed");

    assert_eq!(
        store::list::<model::Skill>(database.connection())
            .expect("list")
            .len(),
        2
    );
}

#[test]
fn seeding_opens_an_initial_goal_version() {
    let seed = seed_file(VALID);
    let mut database = database();
    seed::populate(&mut database, seed.path(), false).expect("seed");

    let versions = store::list::<model::GoalVersion>(database.connection()).expect("list");
    assert_eq!(versions.len(), 1);
    assert!(versions[0].valid_until.is_none());
}

#[test]
fn unknown_references_are_rejected() {
    let invalid = VALID.replace(r#"skills = ["skill-1"]"#, r#"skills = ["missing"]"#);
    let seed = seed_file(&invalid);
    let mut database = database();
    assert!(seed::populate(&mut database, seed.path(), false).is_err());
}

#[test]
fn duplicate_ids_are_rejected() {
    let invalid = VALID.replace(r#"id = "skill-2""#, r#"id = "skill-1""#);
    let seed = seed_file(&invalid);
    let mut database = database();
    assert!(seed::populate(&mut database, seed.path(), false).is_err());
}

#[test]
fn unknown_statuses_are_rejected() {
    let invalid = VALID.replacen(r#"status = "active""#, r#"status = "bogus""#, 1);
    let seed = seed_file(&invalid);
    let mut database = database();
    assert!(seed::populate(&mut database, seed.path(), false).is_err());
}

#[test]
fn out_of_range_skill_levels_are_rejected() {
    let invalid = VALID.replace("level = 7", "level = 99");
    let seed = seed_file(&invalid);
    let mut database = database();
    assert!(seed::populate(&mut database, seed.path(), false).is_err());
}

#[test]
fn self_dependencies_are_rejected() {
    let invalid = VALID.replace("depends_on = []", r#"depends_on = ["milestone-1"]"#);
    let seed = seed_file(&invalid);
    let mut database = database();
    assert!(seed::populate(&mut database, seed.path(), false).is_err());
}
