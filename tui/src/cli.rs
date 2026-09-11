//! Command line interface for writing data.

use clap::{Args, Parser, Subcommand};
use goal_tracker_core::rusqlite::Connection;
use goal_tracker_core::store::{self, Database};
use goal_tracker_core::{Result, export, model, seed};

use crate::form;

/// Track a multi-year engineering roadmap.
#[derive(Debug, Parser)]
#[command(name = "goal-tracker")]
pub(crate) struct Cli {
    /// Database file.
    #[arg(short, long, global = true, default_value = "goal-tracker.db")]
    pub(crate) db: std::path::PathBuf,

    /// Run without a command to open the terminal UI.
    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Add a record.
    Add(Add),
    /// Link two records.
    Link(Link),
    /// Populate the initial roadmap hypothesis.
    Seed(SeedArgs),
    /// Write every record to a text file.
    Export(ExportArgs),
}

#[derive(Debug, Args)]
pub(crate) struct SeedArgs {
    /// Seed file (TOML).
    #[arg(short, long, default_value = "roadmap.seed.toml")]
    file: std::path::PathBuf,

    /// Delete existing records before seeding.
    #[arg(long)]
    force: bool,
}

#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    /// Destination text file.
    #[arg(short, long, default_value = "roadmap.txt")]
    output: std::path::PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct Add {
    #[command(subcommand)]
    entity: AddEntity,
}

#[derive(Debug, Subcommand)]
enum AddEntity {
    /// Add a goal.
    Goal(AddGoal),
    /// Add a skill.
    Skill(AddSkill),
    /// Add a milestone.
    Milestone(AddMilestone),
    /// Add evidence.
    Evidence(AddEvidence),
    /// Add a project.
    Project(AddProject),
    /// Add a decision.
    Decision(AddDecision),
}

#[derive(Debug, Args)]
struct AddGoal {
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long, default_value = "active")]
    status: String,
}

#[derive(Debug, Args)]
struct AddSkill {
    #[arg(long)]
    name: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long, default_value = "active")]
    status: String,
    /// Skill level, 0-10.
    #[arg(long, default_value_t = 0)]
    level: i64,
}

#[derive(Debug, Args)]
struct AddMilestone {
    #[arg(long)]
    title: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long, default_value = "planned")]
    status: String,
}

#[derive(Debug, Args)]
struct AddEvidence {
    #[arg(long)]
    title: String,
    #[arg(long = "type", default_value = "other")]
    evidence_type: String,
    #[arg(long)]
    url: Option<String>,
    #[arg(long)]
    description: Option<String>,
}

#[derive(Debug, Args)]
struct AddProject {
    #[arg(long)]
    name: String,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    repository: Option<String>,
    #[arg(long, default_value = "active")]
    status: String,
}

#[derive(Debug, Args)]
struct AddDecision {
    #[arg(long)]
    description: String,
    #[arg(long)]
    context: String,
    #[arg(long)]
    decision: String,
    #[arg(long)]
    rationale: String,
}

#[derive(Debug, Args)]
pub(crate) struct Link {
    #[command(subcommand)]
    relation: Relation,
}

#[derive(Debug, Subcommand)]
enum Relation {
    /// Attach a goal to a milestone.
    Goal {
        /// Milestone id.
        milestone: String,
        /// Goal id.
        goal: String,
    },
    /// Attach a skill to a milestone.
    Skill {
        /// Milestone id.
        milestone: String,
        /// Skill id.
        skill: String,
    },
    /// Attach evidence to a milestone.
    Evidence {
        /// Milestone id.
        milestone: String,
        /// Evidence id.
        evidence: String,
    },
    /// Attach a project to a milestone.
    Project {
        /// Milestone id.
        milestone: String,
        /// Project id.
        project: String,
    },
    /// Declare that a milestone depends on another.
    Dependency {
        /// Milestone id.
        milestone: String,
        /// Prerequisite milestone id.
        prerequisite: String,
    },
    /// Relate two skills.
    SkillRelationship {
        /// Source skill id.
        from: String,
        /// Target skill id.
        to: String,
        /// Relationship label (for example `encompasses`).
        relationship: String,
    },
}

/// Executes a write command against the database.
pub(crate) fn run(database: &mut Database, command: Command) -> Result<()> {
    match command {
        Command::Add(add) => add.run(database),
        Command::Link(link) => link.run(database.connection()),
        Command::Seed(args) => args.run(database),
        Command::Export(args) => args.run(database.connection()),
    }
}

impl ExportArgs {
    fn run(self, connection: &Connection) -> Result<()> {
        let text = export::to_text(connection)?;
        std::fs::write(&self.output, text)?;
        println!("exported to {}", self.output.display());
        Ok(())
    }
}

impl SeedArgs {
    fn run(self, database: &mut Database) -> Result<()> {
        match seed::populate(database, &self.file, self.force)? {
            Some(report) => println!("seeded {report}"),
            None => println!("database already contains the roadmap (use --force to reset)"),
        }
        Ok(())
    }
}

impl Add {
    fn run(self, database: &mut Database) -> Result<()> {
        match self.entity {
            AddEntity::Goal(args) => args.run(database),
            AddEntity::Skill(args) => args.run(database),
            AddEntity::Milestone(args) => args.run(database),
            AddEntity::Evidence(args) => args.run(database),
            AddEntity::Project(args) => args.run(database),
            AddEntity::Decision(args) => args.run(database),
        }
    }
}

fn report(id: &str) {
    println!("{id}");
}

impl AddGoal {
    fn run(self, database: &mut Database) -> Result<()> {
        let values = vec![
            self.title,
            self.description.unwrap_or_default(),
            self.status,
        ];
        report(&form::save::<model::Goal>(database, None, &values)?);
        Ok(())
    }
}

impl AddSkill {
    fn run(self, database: &mut Database) -> Result<()> {
        let values = vec![
            self.name,
            self.description.unwrap_or_default(),
            self.status,
            self.level.to_string(),
        ];
        report(&form::save::<model::Skill>(database, None, &values)?);
        Ok(())
    }
}

impl AddMilestone {
    fn run(self, database: &mut Database) -> Result<()> {
        let values = vec![
            self.title,
            self.description.unwrap_or_default(),
            self.status,
            String::new(),
        ];
        report(&form::save::<model::Milestone>(database, None, &values)?);
        Ok(())
    }
}

impl AddEvidence {
    fn run(self, database: &mut Database) -> Result<()> {
        let values = vec![
            self.title,
            self.description.unwrap_or_default(),
            self.evidence_type,
            self.url.unwrap_or_default(),
        ];
        report(&form::save::<model::Evidence>(database, None, &values)?);
        Ok(())
    }
}

impl AddProject {
    fn run(self, database: &mut Database) -> Result<()> {
        let values = vec![
            self.name,
            self.description.unwrap_or_default(),
            self.repository.unwrap_or_default(),
            self.status,
        ];
        report(&form::save::<model::Project>(database, None, &values)?);
        Ok(())
    }
}

impl AddDecision {
    fn run(self, database: &mut Database) -> Result<()> {
        let values = vec![
            self.description,
            self.context,
            self.decision,
            self.rationale,
        ];
        report(&form::save::<model::Decision>(database, None, &values)?);
        Ok(())
    }
}

impl Link {
    fn run(self, connection: &Connection) -> Result<()> {
        self.relation.run(connection)
    }
}

impl Relation {
    #[allow(clippy::too_many_lines)]
    fn run(self, connection: &Connection) -> Result<()> {
        match self {
            Self::Goal { milestone, goal } => store::insert(
                connection,
                &model::MilestoneGoal {
                    milestone_id: milestone,
                    goal_id: goal,
                },
            )?,
            Self::Skill { milestone, skill } => store::insert(
                connection,
                &model::MilestoneSkill {
                    milestone_id: milestone,
                    skill_id: skill,
                },
            )?,
            Self::Evidence {
                milestone,
                evidence,
            } => store::insert(
                connection,
                &model::MilestoneEvidence {
                    milestone_id: milestone,
                    evidence_id: evidence,
                },
            )?,
            Self::Project { milestone, project } => store::insert(
                connection,
                &model::MilestoneProject {
                    milestone_id: milestone,
                    project_id: project,
                },
            )?,
            Self::Dependency {
                milestone,
                prerequisite,
            } => store::insert(
                connection,
                &model::MilestoneDependency {
                    milestone_id: milestone,
                    prerequisite_milestone_id: prerequisite,
                },
            )?,
            Self::SkillRelationship {
                from,
                to,
                relationship,
            } => store::insert(
                connection,
                &model::SkillRelationship {
                    from_skill_id: from,
                    to_skill_id: to,
                    relationship,
                },
            )?,
        }
        Ok(())
    }
}
