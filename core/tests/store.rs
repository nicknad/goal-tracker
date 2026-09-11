//! Store round-trips, filtering, mutation and transaction behaviour.

use goal_tracker_core::model::{
    Decision, Evidence, EvidenceType, Goal, GoalStatus, GoalVersion, Milestone,
    MilestoneDependency, MilestoneEvidence, MilestoneGoal, MilestoneProject, MilestoneSkill,
    MilestoneStatus, Project, ProjectStatus, Skill, SkillRelationship, SkillStatus,
};
use goal_tracker_core::rusqlite::types::Value;
use goal_tracker_core::store::{self, Database, Identified};
use goal_tracker_core::{Error, Result};

fn database() -> Database {
    Database::open_in_memory().expect("in-memory database")
}

fn goal() -> Goal {
    Goal {
        id: "goal-1".to_owned(),
        title: "Become a systems engineer".to_owned(),
        description: Some("description".to_owned()),
        status: GoalStatus::Active,
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        archived_at: None,
    }
}

fn goal_version() -> GoalVersion {
    GoalVersion {
        id: "goal-version-1".to_owned(),
        goal_id: "goal-1".to_owned(),
        title: "Become a systems engineer".to_owned(),
        description: Some("description".to_owned()),
        valid_from: "2026-01-01T00:00:00Z".to_owned(),
        valid_until: None,
    }
}

fn skill() -> Skill {
    Skill {
        id: "skill-1".to_owned(),
        name: "Rust".to_owned(),
        description: Some("Systems language".to_owned()),
        status: SkillStatus::Learning,
        level: 5,
        created_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn milestone() -> Milestone {
    Milestone {
        id: "milestone-1".to_owned(),
        title: "Ship a component".to_owned(),
        description: Some("criteria".to_owned()),
        status: MilestoneStatus::Planned,
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        completed_at: None,
        archived_at: None,
    }
}

fn evidence() -> Evidence {
    Evidence {
        id: "evidence-1".to_owned(),
        title: "Repository".to_owned(),
        description: None,
        evidence_type: EvidenceType::Repository,
        url: Some("https://example.invalid".to_owned()),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn decision() -> Decision {
    Decision {
        id: "decision-1".to_owned(),
        description: "Prioritize Rust".to_owned(),
        context: "Context".to_owned(),
        decision: "Decision".to_owned(),
        rationale: "Rationale".to_owned(),
        created_at: "2026-01-01T00:00:00Z".to_owned(),
    }
}

fn project() -> Project {
    Project {
        id: "project-1".to_owned(),
        name: "OTel SQLite".to_owned(),
        description: Some("Telemetry ingestion".to_owned()),
        repository_url: None,
        status: ProjectStatus::Active,
        created_at: "2026-01-01T00:00:00Z".to_owned(),
        archived_at: None,
    }
}

fn round_trip<R>(database: &Database, record: &R)
where
    R: Identified + PartialEq + std::fmt::Debug,
{
    let connection = database.connection();
    store::insert(connection, record).expect("insert");
    let stored = store::get::<R>(connection, record.id())
        .expect("get")
        .expect("stored record");
    assert_eq!(&stored, record);
}

#[test]
fn every_identified_record_round_trips() {
    let database = database();
    round_trip(&database, &goal());
    round_trip(&database, &goal_version());
    round_trip(&database, &skill());
    round_trip(&database, &milestone());
    round_trip(&database, &evidence());
    round_trip(&database, &decision());
    round_trip(&database, &project());
}

#[test]
fn join_tables_round_trip() {
    let database = database();
    let connection = database.connection();
    store::insert(connection, &goal()).expect("goal");
    store::insert(connection, &skill()).expect("skill");
    store::insert(connection, &milestone()).expect("milestone");
    store::insert(connection, &evidence()).expect("evidence");
    store::insert(connection, &project()).expect("project");

    let skill_relationship = SkillRelationship {
        from_skill_id: "skill-1".to_owned(),
        to_skill_id: "skill-1".to_owned(),
        relationship: "related".to_owned(),
    };
    let milestone_skill = MilestoneSkill {
        milestone_id: "milestone-1".to_owned(),
        skill_id: "skill-1".to_owned(),
    };
    let milestone_goal = MilestoneGoal {
        milestone_id: "milestone-1".to_owned(),
        goal_id: "goal-1".to_owned(),
    };
    let milestone_evidence = MilestoneEvidence {
        milestone_id: "milestone-1".to_owned(),
        evidence_id: "evidence-1".to_owned(),
    };
    let milestone_project = MilestoneProject {
        milestone_id: "milestone-1".to_owned(),
        project_id: "project-1".to_owned(),
    };
    let milestone_dependency = MilestoneDependency {
        milestone_id: "milestone-1".to_owned(),
        prerequisite_milestone_id: "milestone-1".to_owned(),
    };

    store::insert(connection, &skill_relationship).expect("skill relationship");
    store::insert(connection, &milestone_skill).expect("milestone skill");
    store::insert(connection, &milestone_goal).expect("milestone goal");
    store::insert(connection, &milestone_evidence).expect("milestone evidence");
    store::insert(connection, &milestone_project).expect("milestone project");
    store::insert(connection, &milestone_dependency).expect("milestone dependency");

    assert_eq!(
        store::list::<SkillRelationship>(connection).expect("list"),
        vec![skill_relationship]
    );
    assert_eq!(
        store::list::<MilestoneSkill>(connection).expect("list"),
        vec![milestone_skill]
    );
    assert_eq!(
        store::list::<MilestoneGoal>(connection).expect("list"),
        vec![milestone_goal]
    );
    assert_eq!(
        store::list::<MilestoneEvidence>(connection).expect("list"),
        vec![milestone_evidence]
    );
    assert_eq!(
        store::list::<MilestoneProject>(connection).expect("list"),
        vec![milestone_project]
    );
    assert_eq!(
        store::list::<MilestoneDependency>(connection).expect("list"),
        vec![milestone_dependency]
    );
}

#[test]
fn update_and_delete_touch_one_row() {
    let database = database();
    let connection = database.connection();
    let mut goal = goal();
    store::insert(connection, &goal).expect("insert");

    goal.title = "Updated".to_owned();
    assert_eq!(store::update(connection, &goal).expect("update"), 1);
    let stored = store::get::<Goal>(connection, "goal-1")
        .expect("get")
        .expect("stored");
    assert_eq!(stored.title, "Updated");

    assert_eq!(
        store::delete::<Goal>(connection, "goal-1").expect("delete"),
        1
    );
    assert!(
        store::get::<Goal>(connection, "goal-1")
            .expect("get")
            .is_none()
    );
}

#[test]
fn deleting_a_skill_cascades_its_links() {
    let database = database();
    let connection = database.connection();
    store::insert(connection, &skill()).expect("skill");
    store::insert(connection, &milestone()).expect("milestone");

    let milestone_skill = MilestoneSkill {
        milestone_id: "milestone-1".to_owned(),
        skill_id: "skill-1".to_owned(),
    };
    let skill_relationship = SkillRelationship {
        from_skill_id: "skill-1".to_owned(),
        to_skill_id: "skill-1".to_owned(),
        relationship: "related".to_owned(),
    };
    store::insert(connection, &milestone_skill).expect("milestone skill");
    store::insert(connection, &skill_relationship).expect("skill relationship");

    assert_eq!(
        store::delete::<Skill>(connection, "skill-1").expect("delete"),
        1
    );
    assert!(
        store::list::<MilestoneSkill>(connection)
            .expect("list")
            .is_empty()
    );
    assert!(
        store::list::<SkillRelationship>(connection)
            .expect("list")
            .is_empty()
    );
}

#[test]
fn list_where_filters_and_deletes() {
    let database = database();
    let connection = database.connection();
    let mut paused = goal();
    paused.id = "goal-2".to_owned();
    paused.status = GoalStatus::Paused;
    store::insert(connection, &goal()).expect("insert");
    store::insert(connection, &paused).expect("insert");

    let active =
        store::list_where::<Goal>(connection, &[("status", Value::Text("active".to_owned()))])
            .expect("list");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, "goal-1");

    let deleted =
        store::delete_where::<Goal>(connection, &[("status", Value::Text("paused".to_owned()))])
            .expect("delete");
    assert_eq!(deleted, 1);
    assert_eq!(store::list::<Goal>(connection).expect("list").len(), 1);
}

#[test]
fn transactions_commit_or_roll_back() {
    let mut database = database();
    database
        .with_transaction(|connection| {
            store::insert(connection, &goal())?;
            Ok(())
        })
        .expect("commit");
    assert_eq!(
        store::list::<Goal>(database.connection())
            .expect("list")
            .len(),
        1
    );

    let result: Result<()> = database.with_transaction(|connection| {
        store::insert(connection, &goal_version())?;
        Err(Error::Schema("intentional rollback".to_owned()))
    });
    assert!(result.is_err());
    assert!(
        store::list::<GoalVersion>(database.connection())
            .expect("list")
            .is_empty()
    );
}
