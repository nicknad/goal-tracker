//! Domain model.
//!
//! Goals and skills are the current hypothesis and may be restructured freely.
//! Milestones, evidence and decisions are historical records and are never
//! rewritten when the hypothesis changes.

/// Generates a string-backed enum with SQLite conversion support.
macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $(
                #[doc = concat!("Stored as `", $text, "`.")]
                $variant
            ),+
        }

        impl $name {
            /// Canonical strings, in declaration order.
            pub const ALL: &'static [&'static str] = &[$($text),+];

            /// Canonical string stored in the database.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::error::Error;

            fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
                match value {
                    $($text => Ok(Self::$variant),)+
                    other => Err($crate::error::Error::InvalidValue {
                        kind: stringify!($name),
                        value: other.to_owned(),
                    }),
                }
            }
        }

        impl $crate::rusqlite::types::FromSql for $name {
            fn column_result(
                value: $crate::rusqlite::types::ValueRef<'_>,
            ) -> $crate::rusqlite::types::FromSqlResult<Self> {
                value.as_str().and_then(|text| {
                    text.parse().map_err(|error| {
                        $crate::rusqlite::types::FromSqlError::Other(::std::boxed::Box::new(error))
                    })
                })
            }
        }

        impl $crate::store::FieldValue for $name {
            fn to_value(&self) -> $crate::rusqlite::types::Value {
                $crate::rusqlite::types::Value::Text(self.as_str().to_owned())
            }
        }
    };
}

/// Returns the column name for a field, allowing an explicit override.
#[doc(hidden)]
#[macro_export]
macro_rules! __column {
    ($field:ident) => {
        stringify!($field)
    };
    ($field:ident as $column:literal) => {
        $column
    };
}

/// Implements [`Record`](crate::store::Record) for a struct.
///
/// Prefix the field list with `identified` when the struct has an `id` field
/// and should implement [`Identified`](crate::store::Identified) as well.
macro_rules! impl_record {
    ($name:ident, $table:literal, identified [$($field:ident $(as $column:literal)? : $ty:ty),+ $(,)?]) => {
        impl_record!(@impl $name, $table, [$($field $(as $column)? : $ty),+]);

        impl $crate::store::Identified for $name {
            fn id(&self) -> &str {
                &self.id
            }
        }
    };
    ($name:ident, $table:literal, [$($field:ident $(as $column:literal)? : $ty:ty),+ $(,)?]) => {
        impl_record!(@impl $name, $table, [$($field $(as $column)? : $ty),+]);
    };
    (@impl $name:ident, $table:literal, [$($field:ident $(as $column:literal)? : $ty:ty),+ $(,)?]) => {
        impl $crate::store::Record for $name {
            const TABLE: &'static str = $table;
            const COLUMNS: &'static [&'static str] =
                &[$($crate::__column!($field $(as $column)?)),+];

            fn params(
                &self,
            ) -> ::std::vec::Vec<(&'static str, $crate::rusqlite::types::Value)> {
                vec![$(
                    (
                        $crate::__column!($field $(as $column)?),
                        $crate::store::FieldValue::to_value(&self.$field),
                    )
                ),+]
            }

            fn from_row(
                row: &$crate::rusqlite::Row<'_>,
            ) -> $crate::rusqlite::Result<Self> {
                Ok(Self {
                    $(
                        $field: row.get::<_, $ty>($crate::__column!($field $(as $column)?))?,
                    )+
                })
            }
        }
    };
}

string_enum!(
    /// Lifecycle state of a goal.
    GoalStatus {
        Active => "active",
        Paused => "paused",
        Achieved => "achieved",
        Abandoned => "abandoned",
        Archived => "archived",
    }
);

string_enum!(
    /// Lifecycle state of a skill.
    SkillStatus {
        Active => "active",
        Learning => "learning",
        Paused => "paused",
        Archived => "archived",
    }
);

string_enum!(
    /// Lifecycle state of a milestone.
    MilestoneStatus {
        Planned => "planned",
        InProgress => "in_progress",
        Blocked => "blocked",
        Completed => "completed",
        Archived => "archived",
    }
);

string_enum!(
    /// Lifecycle state of a project.
    ProjectStatus {
        Active => "active",
        Paused => "paused",
        Completed => "completed",
        Archived => "archived",
        Planned => "planned",
    }
);

string_enum!(
    /// Kind of evidence attached to a milestone.
    EvidenceType {
        Repository => "repository",
        PullRequest => "pull_request",
        BlogPost => "blog_post",
        Benchmark => "benchmark",
        Production => "production",
        ArchitectureDocument => "architecture_document",
        ConferenceTalk => "conference_talk",
        CodeReview => "code_review",
        Certification => "certification",
        Other => "other",
    }
);

/// A desired outcome. Identity survives while its definition evolves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: GoalStatus,
    pub created_at: String,
    pub archived_at: Option<String>,
}

/// A point-in-time definition of a [`Goal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalVersion {
    pub id: String,
    pub goal_id: String,
    pub title: String,
    pub description: Option<String>,
    pub valid_from: String,
    pub valid_until: Option<String>,
}

/// Highest skill level. Levels are integers from 0 to `MAX_LEVEL`.
pub const MAX_LEVEL: i64 = 10;

/// A capability, deliberately without a fixed hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub status: SkillStatus,
    pub level: i64,
    pub created_at: String,
}

/// A directed relationship between two skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRelationship {
    pub from_skill_id: String,
    pub to_skill_id: String,
    pub relationship: String,
}

/// Something accomplishable and demonstrable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: MilestoneStatus,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub archived_at: Option<String>,
}

/// Associates a milestone with a skill it demonstrates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneSkill {
    pub milestone_id: String,
    pub skill_id: String,
}

/// Associates a milestone with a goal it motivates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneGoal {
    pub milestone_id: String,
    pub goal_id: String,
}

/// Declares that a milestone depends on another milestone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneDependency {
    pub milestone_id: String,
    pub prerequisite_milestone_id: String,
}

/// Proof that a milestone was achieved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub evidence_type: EvidenceType,
    pub url: Option<String>,
    pub created_at: String,
}

/// Associates a milestone with its evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneEvidence {
    pub milestone_id: String,
    pub evidence_id: String,
}

/// A journal entry recording why something was decided. Standalone: decisions
/// reference nothing and nothing references them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    pub description: String,
    pub context: String,
    pub decision: String,
    pub rationale: String,
    pub created_at: String,
}

/// A body of work through which milestones happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub repository_url: Option<String>,
    pub status: ProjectStatus,
    pub created_at: String,
    pub archived_at: Option<String>,
}

/// Associates a milestone with a project that implements it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MilestoneProject {
    pub milestone_id: String,
    pub project_id: String,
}

impl_record!(Goal, "goals", identified [
    id: String,
    title: String,
    description: Option<String>,
    status: GoalStatus,
    created_at: String,
    archived_at: Option<String>,
]);

impl_record!(GoalVersion, "goal_versions", identified [
    id: String,
    goal_id: String,
    title: String,
    description: Option<String>,
    valid_from: String,
    valid_until: Option<String>,
]);

impl_record!(Skill, "skills", identified [
    id: String,
    name: String,
    description: Option<String>,
    status: SkillStatus,
    level: i64,
    created_at: String,
]);

impl_record!(SkillRelationship, "skill_relationships", [
    from_skill_id: String,
    to_skill_id: String,
    relationship: String,
]);

impl_record!(Milestone, "milestones", identified [
    id: String,
    title: String,
    description: Option<String>,
    status: MilestoneStatus,
    created_at: String,
    completed_at: Option<String>,
    archived_at: Option<String>,
]);

impl_record!(MilestoneSkill, "milestone_skills", [
    milestone_id: String,
    skill_id: String,
]);

impl_record!(MilestoneGoal, "milestone_goals", [
    milestone_id: String,
    goal_id: String,
]);

impl_record!(MilestoneDependency, "milestone_dependencies", [
    milestone_id: String,
    prerequisite_milestone_id: String,
]);

impl_record!(Evidence, "evidence", identified [
    id: String,
    title: String,
    description: Option<String>,
    evidence_type as "type": EvidenceType,
    url: Option<String>,
    created_at: String,
]);

impl_record!(MilestoneEvidence, "milestone_evidence", [
    milestone_id: String,
    evidence_id: String,
]);

impl_record!(Decision, "decisions", identified [
    id: String,
    description: String,
    context: String,
    decision: String,
    rationale: String,
    created_at: String,
]);

impl_record!(Project, "projects", identified [
    id: String,
    name: String,
    description: Option<String>,
    repository_url: Option<String>,
    status: ProjectStatus,
    created_at: String,
    archived_at: Option<String>,
]);

impl_record!(MilestoneProject, "milestone_projects", [
    milestone_id: String,
    project_id: String,
]);

/// Checks that every table declared by the model exists and has the expected
/// columns. Called when a [`Database`](crate::store::Database) is opened, so
/// schema drift fails immediately instead of at the first query.
pub(crate) fn verify_schema(connection: &crate::rusqlite::Connection) -> crate::Result<()> {
    fn check<R: crate::store::Record>(
        connection: &crate::rusqlite::Connection,
    ) -> crate::Result<()> {
        let mut statement = connection.prepare(&format!("PRAGMA table_info({})", R::TABLE))?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<crate::rusqlite::Result<Vec<String>>>()?;
        let missing = R::COLUMNS
            .iter()
            .filter(|column| !columns.iter().any(|existing| existing.as_str() == **column))
            .copied()
            .collect::<Vec<_>>();
        if missing.is_empty() {
            return Ok(());
        }
        Err(crate::Error::Schema(format!(
            "table '{}' is missing columns: {}",
            R::TABLE,
            missing.join(", ")
        )))
    }

    check::<Goal>(connection)?;
    check::<GoalVersion>(connection)?;
    check::<Skill>(connection)?;
    check::<SkillRelationship>(connection)?;
    check::<Milestone>(connection)?;
    check::<MilestoneSkill>(connection)?;
    check::<MilestoneGoal>(connection)?;
    check::<MilestoneDependency>(connection)?;
    check::<Evidence>(connection)?;
    check::<MilestoneEvidence>(connection)?;
    check::<Decision>(connection)?;
    check::<Project>(connection)?;
    check::<MilestoneProject>(connection)?;
    Ok(())
}

/// Current UTC time as an RFC 3339 string.
#[must_use]
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Generates a unique identifier with the given prefix.
#[must_use]
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", uuid::Uuid::new_v4())
}
