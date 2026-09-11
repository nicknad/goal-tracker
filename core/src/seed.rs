//! Seeding the database from a TOML file.
//!
//! The seed file is personal data and lives outside version control. It is
//! validated both structurally (types and required fields, via serde) and
//! semantically (unique ids, resolvable references, parseable statuses) before
//! any database write. Seeding is idempotent per goal id unless `force` is set.
//!
//! Milestone `criteria`, `evidence` and `validation` are prose fields rendered
//! into the milestone description. The `evidence` table holds actual proof
//! linked to a milestone after the fact, which the seed cannot fabricate.

use std::collections::HashSet;
use std::fmt;
use std::path::Path;

use serde::Deserialize;

use crate::model::{
    self, Decision, Goal, Milestone, MilestoneDependency, MilestoneGoal, MilestoneProject,
    MilestoneSkill, MilestoneStatus, Project, ProjectStatus, Skill, SkillRelationship, SkillStatus,
};
use crate::rusqlite::Connection;
use crate::{Database, Error, Result, history, store};

/// Summary of what a successful seed inserted.
#[derive(Debug, Clone, Copy)]
pub struct SeedReport {
    pub goals: usize,
    pub skills: usize,
    pub milestones: usize,
    pub projects: usize,
}

impl fmt::Display for SeedReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} goal, {} skills, {} milestones, {} projects",
            self.goals, self.skills, self.milestones, self.projects
        )
    }
}

#[derive(Debug, Deserialize)]
struct SeedFile {
    goal: GoalSeed,
    decision: DecisionSeed,
    #[serde(default)]
    skills: Vec<SkillSeed>,
    #[serde(default)]
    milestones: Vec<MilestoneSeed>,
    #[serde(default)]
    projects: Vec<ProjectSeed>,
    #[serde(default)]
    skill_relationships: Vec<SkillRelationshipSeed>,
}

#[derive(Debug, Deserialize)]
struct GoalSeed {
    id: String,
    title: String,
    description: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct SkillSeed {
    id: String,
    name: String,
    description: String,
    status: String,
    #[serde(default)]
    level: i64,
}

#[derive(Debug, Deserialize)]
struct MilestoneSeed {
    id: String,
    title: String,
    status: String,
    criteria: Vec<String>,
    evidence: String,
    validation: String,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    projects: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProjectSeed {
    id: String,
    name: String,
    description: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct DecisionSeed {
    id: String,
    #[serde(alias = "title")]
    description: String,
    context: String,
    decision: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct SkillRelationshipSeed {
    from: String,
    to: String,
    relationship: String,
}

/// Loads `path`, validates it and inserts it in a single transaction.
///
/// Returns `None` when the goal already exists and `force` is not set.
pub fn populate(
    database: &mut Database,
    path: impl AsRef<Path>,
    force: bool,
) -> Result<Option<SeedReport>> {
    let seed = SeedFile::load(path.as_ref())?;
    if !force && store::get::<Goal>(database.connection(), &seed.goal.id)?.is_some() {
        return Ok(None);
    }
    let report = database.with_transaction(|connection| {
        if force {
            store::reset(connection)?;
        }
        seed.insert(connection)?;
        Ok(SeedReport {
            goals: 1,
            skills: seed.skills.len(),
            milestones: seed.milestones.len(),
            projects: seed.projects.len(),
        })
    })?;
    Ok(Some(report))
}

impl SeedFile {
    fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| Error::Seed(format!("{}: {error}", path.display())))?;
        let seed: Self = toml::from_str(&text)
            .map_err(|error| Error::Seed(format!("{}: {error}", path.display())))?;
        seed.validate()?;
        Ok(seed)
    }

    /// Checks uniqueness and that every reference resolves.
    fn validate(&self) -> Result<()> {
        self.goal.status.parse::<model::GoalStatus>()?;
        let skill_ids = unique_ids("skill", self.skills.iter().map(|skill| skill.id.as_str()))?;
        let project_ids = unique_ids(
            "project",
            self.projects.iter().map(|project| project.id.as_str()),
        )?;
        let milestone_ids = unique_ids(
            "milestone",
            self.milestones
                .iter()
                .map(|milestone| milestone.id.as_str()),
        )?;

        for skill in &self.skills {
            skill.status.parse::<SkillStatus>()?;
            if !(0..=model::MAX_LEVEL).contains(&skill.level) {
                return Err(Error::Seed(format!(
                    "skill '{}' has level {} outside 0-{}",
                    skill.id,
                    skill.level,
                    model::MAX_LEVEL
                )));
            }
        }
        for project in &self.projects {
            project.status.parse::<ProjectStatus>()?;
        }
        for milestone in &self.milestones {
            milestone.status.parse::<MilestoneStatus>()?;
            for skill in &milestone.skills {
                if !skill_ids.contains(skill.as_str()) {
                    return Err(Error::Seed(format!(
                        "milestone '{}' references unknown skill '{skill}'",
                        milestone.id
                    )));
                }
            }
            for project in &milestone.projects {
                if !project_ids.contains(project.as_str()) {
                    return Err(Error::Seed(format!(
                        "milestone '{}' references unknown project '{project}'",
                        milestone.id
                    )));
                }
            }
            for dependency in &milestone.depends_on {
                if dependency == &milestone.id {
                    return Err(Error::Seed(format!(
                        "milestone '{}' depends on itself",
                        milestone.id
                    )));
                }
                if !milestone_ids.contains(dependency.as_str()) {
                    return Err(Error::Seed(format!(
                        "milestone '{}' references unknown prerequisite '{dependency}'",
                        milestone.id
                    )));
                }
            }
        }
        for relationship in &self.skill_relationships {
            if !skill_ids.contains(relationship.from.as_str())
                || !skill_ids.contains(relationship.to.as_str())
            {
                return Err(Error::Seed(format!(
                    "skill relationship '{}' -> '{}' references an unknown skill",
                    relationship.from, relationship.to
                )));
            }
        }
        Ok(())
    }

    fn insert(&self, connection: &Connection) -> Result<()> {
        let timestamp = model::now();
        let goal = self.goal_model(&timestamp);
        store::insert(connection, &goal)?;
        history::revise_goal(connection, &goal)?;

        for skill in &self.skills {
            store::insert(connection, &skill_model(skill, &timestamp)?)?;
        }
        for milestone in &self.milestones {
            store::insert(connection, &milestone_model(milestone, &timestamp)?)?;
        }
        for project in &self.projects {
            store::insert(connection, &project_model(project, &timestamp)?)?;
        }
        store::insert(connection, &self.decision_model(&timestamp))?;

        for milestone in &self.milestones {
            link_goal(connection, &milestone.id, &self.goal.id)?;
            for skill in &milestone.skills {
                link_skill(connection, &milestone.id, skill)?;
            }
            for project in &milestone.projects {
                link_project(connection, &milestone.id, project)?;
            }
            for dependency in &milestone.depends_on {
                depend(connection, &milestone.id, dependency)?;
            }
        }
        for relationship in &self.skill_relationships {
            store::insert(
                connection,
                &SkillRelationship {
                    from_skill_id: relationship.from.clone(),
                    to_skill_id: relationship.to.clone(),
                    relationship: relationship.relationship.clone(),
                },
            )?;
        }
        Ok(())
    }

    fn goal_model(&self, timestamp: &str) -> Goal {
        Goal {
            id: self.goal.id.clone(),
            title: self.goal.title.clone(),
            description: Some(self.goal.description.clone()),
            status: self
                .goal
                .status
                .parse()
                .expect("goal status validated before insert"),
            created_at: timestamp.to_owned(),
            archived_at: None,
        }
    }

    fn decision_model(&self, timestamp: &str) -> Decision {
        Decision {
            id: self.decision.id.clone(),
            description: self.decision.description.clone(),
            context: self.decision.context.clone(),
            decision: self.decision.decision.clone(),
            rationale: self.decision.rationale.clone(),
            created_at: timestamp.to_owned(),
        }
    }
}

fn unique_ids<'a>(kind: &str, ids: impl Iterator<Item = &'a str>) -> Result<HashSet<&'a str>> {
    let mut seen = HashSet::new();
    for id in ids {
        if id.is_empty() {
            return Err(Error::Seed(format!("{kind} id must not be empty")));
        }
        if !seen.insert(id) {
            return Err(Error::Seed(format!("duplicate {kind} id '{id}'")));
        }
    }
    Ok(seen)
}

fn skill_model(seed: &SkillSeed, timestamp: &str) -> Result<Skill> {
    Ok(Skill {
        id: seed.id.clone(),
        name: seed.name.clone(),
        description: Some(seed.description.clone()),
        status: seed.status.parse()?,
        level: seed.level,
        created_at: timestamp.to_owned(),
    })
}

fn milestone_model(seed: &MilestoneSeed, timestamp: &str) -> Result<Milestone> {
    let criteria = seed
        .criteria
        .iter()
        .map(|criterion| format!("- {criterion}"))
        .collect::<Vec<_>>()
        .join("\n");
    let description = format!(
        "Completion criteria:\n{criteria}\nEvidence expected: {}\nValidation: {}",
        seed.evidence, seed.validation
    );
    Ok(Milestone {
        id: seed.id.clone(),
        title: seed.title.clone(),
        description: Some(description),
        status: seed.status.parse()?,
        created_at: timestamp.to_owned(),
        completed_at: None,
        archived_at: None,
    })
}

fn project_model(seed: &ProjectSeed, timestamp: &str) -> Result<Project> {
    Ok(Project {
        id: seed.id.clone(),
        name: seed.name.clone(),
        description: Some(seed.description.clone()),
        repository_url: None,
        status: seed.status.parse()?,
        created_at: timestamp.to_owned(),
        archived_at: None,
    })
}

fn link_goal(connection: &Connection, milestone_id: &str, goal_id: &str) -> Result<()> {
    store::insert(
        connection,
        &MilestoneGoal {
            milestone_id: milestone_id.to_owned(),
            goal_id: goal_id.to_owned(),
        },
    )
}

fn link_skill(connection: &Connection, milestone_id: &str, skill_id: &str) -> Result<()> {
    store::insert(
        connection,
        &MilestoneSkill {
            milestone_id: milestone_id.to_owned(),
            skill_id: skill_id.to_owned(),
        },
    )
}

fn link_project(connection: &Connection, milestone_id: &str, project_id: &str) -> Result<()> {
    store::insert(
        connection,
        &MilestoneProject {
            milestone_id: milestone_id.to_owned(),
            project_id: project_id.to_owned(),
        },
    )
}

fn depend(connection: &Connection, milestone_id: &str, prerequisite_id: &str) -> Result<()> {
    store::insert(
        connection,
        &MilestoneDependency {
            milestone_id: milestone_id.to_owned(),
            prerequisite_milestone_id: prerequisite_id.to_owned(),
        },
    )
}
