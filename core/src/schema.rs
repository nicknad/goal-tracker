//! SQLite schema and ordered migrations.
//!
//! Migrations are applied by [`Database::open`](crate::store::Database::open)
//! using `PRAGMA user_version`. The project is unpublished, so the history is a
//! single baseline schema; append new statements to [`MIGRATIONS`] as the model
//! evolves.

/// Version 1: the baseline schema.
pub const SCHEMA: &str = r"
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS goals (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    archived_at TEXT
);

CREATE TABLE IF NOT EXISTS goal_versions (
    id          TEXT PRIMARY KEY,
    goal_id     TEXT NOT NULL,
    title       TEXT NOT NULL,
    description TEXT,
    valid_from  TEXT NOT NULL,
    valid_until TEXT,
    FOREIGN KEY (goal_id) REFERENCES goals(id)
);

CREATE TABLE IF NOT EXISTS skills (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    status      TEXT NOT NULL,
    level       INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS skill_relationships (
    from_skill_id TEXT NOT NULL,
    to_skill_id   TEXT NOT NULL,
    relationship  TEXT NOT NULL,
    PRIMARY KEY (from_skill_id, to_skill_id, relationship),
    FOREIGN KEY (from_skill_id) REFERENCES skills(id) ON DELETE CASCADE,
    FOREIGN KEY (to_skill_id) REFERENCES skills(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS milestones (
    id           TEXT PRIMARY KEY,
    title        TEXT NOT NULL,
    description  TEXT,
    status       TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    completed_at TEXT,
    archived_at  TEXT
);

CREATE TABLE IF NOT EXISTS milestone_skills (
    milestone_id TEXT NOT NULL,
    skill_id     TEXT NOT NULL,
    PRIMARY KEY (milestone_id, skill_id),
    FOREIGN KEY (milestone_id) REFERENCES milestones(id),
    FOREIGN KEY (skill_id) REFERENCES skills(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS milestone_goals (
    milestone_id TEXT NOT NULL,
    goal_id      TEXT NOT NULL,
    PRIMARY KEY (milestone_id, goal_id),
    FOREIGN KEY (milestone_id) REFERENCES milestones(id),
    FOREIGN KEY (goal_id) REFERENCES goals(id)
);

CREATE TABLE IF NOT EXISTS milestone_dependencies (
    milestone_id              TEXT NOT NULL,
    prerequisite_milestone_id TEXT NOT NULL,
    PRIMARY KEY (milestone_id, prerequisite_milestone_id),
    FOREIGN KEY (milestone_id) REFERENCES milestones(id),
    FOREIGN KEY (prerequisite_milestone_id) REFERENCES milestones(id)
);

CREATE TABLE IF NOT EXISTS evidence (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT,
    type        TEXT,
    url         TEXT,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS milestone_evidence (
    milestone_id TEXT NOT NULL,
    evidence_id  TEXT NOT NULL,
    PRIMARY KEY (milestone_id, evidence_id),
    FOREIGN KEY (milestone_id) REFERENCES milestones(id),
    FOREIGN KEY (evidence_id) REFERENCES evidence(id)
);

CREATE TABLE IF NOT EXISTS decisions (
    id          TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    context     TEXT NOT NULL,
    decision    TEXT NOT NULL,
    rationale   TEXT NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    description    TEXT,
    repository_url TEXT,
    status         TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    archived_at    TEXT
);

CREATE TABLE IF NOT EXISTS milestone_projects (
    milestone_id TEXT NOT NULL,
    project_id   TEXT NOT NULL,
    PRIMARY KEY (milestone_id, project_id),
    FOREIGN KEY (milestone_id) REFERENCES milestones(id),
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
";

/// Ordered migrations, applied in sequence until `PRAGMA user_version` matches
/// the length of this list.
pub const MIGRATIONS: &[&str] = &[SCHEMA];

/// Tables in foreign-key-safe deletion order (children before parents).
pub const TABLES: &[&str] = &[
    "milestone_goals",
    "milestone_skills",
    "milestone_evidence",
    "milestone_projects",
    "milestone_dependencies",
    "skill_relationships",
    "goal_versions",
    "decisions",
    "evidence",
    "milestones",
    "skills",
    "projects",
    "goals",
];
