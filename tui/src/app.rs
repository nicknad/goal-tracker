//! Application state and input handling.

use std::time::Duration;

use goal_tracker_core::Result;
use goal_tracker_core::model;
use goal_tracker_core::rusqlite::Connection;
use goal_tracker_core::store::{self, Database, Identified};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::form::{self, FormEntity, FormField};
use crate::ui;

const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Width policy for one table column.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ColumnWidth {
    /// Fixed width in columns.
    Length(u16),
    /// Equal share of the space remaining after fixed columns.
    Fill,
    /// Like [`ColumnWidth::Fill`], but never narrower than the given percentage
    /// of the available width.
    FillMin(u16),
}

/// Rows loaded for the current section.
pub(crate) struct Table {
    pub(crate) headers: Vec<&'static str>,
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) ids: Vec<String>,
    pub(crate) details: Vec<Vec<(&'static str, String)>>,
    pub(crate) widths: &'static [ColumnWidth],
}

/// Declares the sections and their backing record types.
///
/// One entry generates the enum, the ordered list and every dispatch function,
/// so adding a section is a single line.
macro_rules! sections {
    ($($variant:ident => ($title:literal, $noun:literal, $model:ty)),+ $(,)?) => {
        /// A top-level view backed by one record type.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum Section {
            $($variant),+
        }

        impl Section {
            pub(crate) const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub(crate) const fn title(self) -> &'static str {
                match self {
                    $(Self::$variant => $title),+
                }
            }

            pub(crate) const fn noun(self) -> &'static str {
                match self {
                    $(Self::$variant => $noun),+
                }
            }

            pub(crate) fn index(self) -> usize {
                Self::ALL
                    .iter()
                    .position(|section| *section == self)
                    .unwrap_or(0)
            }

            fn load(self, connection: &Connection) -> Result<Table> {
                match self {
                    $(Self::$variant => table::<$model>(connection)),+
                }
            }

            fn form_fields(
                self,
                connection: &Connection,
                id: Option<&str>,
            ) -> Result<Vec<FormField>> {
                match self {
                    $(Self::$variant => form::form_fields::<$model>(connection, id)),+
                }
            }

            fn save(
                self,
                database: &mut Database,
                id: Option<&str>,
                values: &[String],
            ) -> Result<()> {
                match self {
                    $(Self::$variant => form::save::<$model>(database, id, values).map(|_| ())),+
                }
            }
        }
    };
}

sections! {
    Goals => ("Goals", "goal", model::Goal),
    Milestones => ("Milestones", "milestone", model::Milestone),
    Decisions => ("Decisions", "decision", model::Decision),
    Skills => ("Skills", "skill", model::Skill),
    Evidence => ("Evidence", "evidence", model::Evidence),
    Projects => ("Projects", "project", model::Project),
}

/// Renders a record as a table row.
pub(crate) trait TableItem: Identified {
    const HEADERS: &'static [&'static str];
    /// Width policy per column, in header order.
    const WIDTHS: &'static [ColumnWidth];

    fn cells(&self) -> Vec<String>;
}

fn table<R: TableItem + FormEntity>(connection: &Connection) -> Result<Table> {
    let records = store::list::<R>(connection)?;
    let rows = records.iter().map(TableItem::cells).collect();
    let ids = records
        .iter()
        .map(|record| record.id().to_owned())
        .collect();
    let details = records
        .iter()
        .map(|record| {
            R::FIELDS
                .iter()
                .zip(record.values())
                .map(|(field, value)| (field.label, value))
                .collect()
        })
        .collect();
    debug_assert_eq!(R::HEADERS.len(), R::WIDTHS.len());
    Ok(Table {
        headers: R::HEADERS.to_vec(),
        rows,
        ids,
        details,
        widths: R::WIDTHS,
    })
}

fn date(timestamp: &str) -> &str {
    timestamp.get(..10).unwrap_or(timestamp)
}

/// Renders a skill level as a bar, for example `██████░░░░ 6/10`.
fn level_bar(level: i64) -> String {
    let max = usize::try_from(model::MAX_LEVEL).unwrap_or(10);
    let filled = usize::try_from(level).unwrap_or(0).min(max);
    let empty = max - filled;
    format!(
        "{}{} {level}/{}",
        "█".repeat(filled),
        "░".repeat(empty),
        model::MAX_LEVEL
    )
}

impl TableItem for model::Goal {
    const HEADERS: &'static [&'static str] = &["Title", "Status", "Created"];
    const WIDTHS: &'static [ColumnWidth] = &[
        ColumnWidth::FillMin(60),
        ColumnWidth::Length(10),
        ColumnWidth::Length(10),
    ];

    fn cells(&self) -> Vec<String> {
        vec![
            self.title.clone(),
            self.status.to_string(),
            date(&self.created_at).to_owned(),
        ]
    }
}

impl TableItem for model::Milestone {
    const HEADERS: &'static [&'static str] = &["Title", "Status", "Completed"];
    const WIDTHS: &'static [ColumnWidth] = &[
        ColumnWidth::FillMin(60),
        ColumnWidth::Length(10),
        ColumnWidth::Length(10),
    ];

    fn cells(&self) -> Vec<String> {
        vec![
            self.title.clone(),
            self.status.to_string(),
            self.completed_at.as_deref().map_or("-", date).to_owned(),
        ]
    }
}

impl TableItem for model::Skill {
    const HEADERS: &'static [&'static str] = &["Name", "Status", "Level", "Created"];
    // Level is wide enough for the full `██████████ 10/10` bar (16 columns).
    const WIDTHS: &'static [ColumnWidth] = &[
        ColumnWidth::FillMin(60),
        ColumnWidth::Length(10),
        ColumnWidth::Length(16),
        ColumnWidth::Length(10),
    ];

    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.status.to_string(),
            level_bar(self.level),
            date(&self.created_at).to_owned(),
        ]
    }
}

impl TableItem for model::Evidence {
    const HEADERS: &'static [&'static str] = &["Title", "Type", "URL"];
    const WIDTHS: &'static [ColumnWidth] =
        &[ColumnWidth::Fill, ColumnWidth::Fill, ColumnWidth::Fill];

    fn cells(&self) -> Vec<String> {
        vec![
            self.title.clone(),
            self.evidence_type.to_string(),
            self.url.as_deref().unwrap_or("-").to_owned(),
        ]
    }
}

impl TableItem for model::Project {
    const HEADERS: &'static [&'static str] = &["Name", "Status", "Repository"];
    const WIDTHS: &'static [ColumnWidth] =
        &[ColumnWidth::Fill, ColumnWidth::Fill, ColumnWidth::Fill];

    fn cells(&self) -> Vec<String> {
        vec![
            self.name.clone(),
            self.status.to_string(),
            self.repository_url.as_deref().unwrap_or("-").to_owned(),
        ]
    }
}

impl TableItem for model::Decision {
    const HEADERS: &'static [&'static str] = &["Description", "Created"];
    const WIDTHS: &'static [ColumnWidth] = &[ColumnWidth::FillMin(60), ColumnWidth::Length(10)];

    fn cells(&self) -> Vec<String> {
        vec![self.description.clone(), date(&self.created_at).to_owned()]
    }
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Next,
    Previous,
}

/// An open add or edit form.
pub(crate) struct Form {
    pub(crate) title: String,
    pub(crate) fields: Vec<FormField>,
    pub(crate) selected: usize,
    id: Option<String>,
    pub(crate) error: Option<String>,
}

impl Form {
    fn move_field(&mut self, direction: Direction) {
        if self.fields.is_empty() {
            return;
        }
        let last = self.fields.len() - 1;
        self.selected = match direction {
            Direction::Next => (self.selected + 1).min(last),
            Direction::Previous => self.selected.saturating_sub(1),
        };
    }

    pub(crate) fn choices(&self) -> &'static [&'static str] {
        self.fields
            .get(self.selected)
            .map_or(&[], |field| field.choices)
    }
}

/// Whether the app is browsing or editing.
pub(crate) enum Mode {
    Browse,
    Form(Form),
}

/// Mutable application state.
pub(crate) struct App {
    database: Database,
    pub(crate) section: Section,
    pub(crate) selected: usize,
    pub(crate) table: Table,
    pub(crate) status: String,
    pub(crate) mode: Mode,
    expanded: bool,
    quit: bool,
}

impl App {
    pub(crate) fn new(database: Database) -> Result<Self> {
        let mut app = Self {
            database,
            section: Section::Goals,
            selected: 0,
            table: Table {
                headers: Vec::new(),
                rows: Vec::new(),
                ids: Vec::new(),
                details: Vec::new(),
                widths: &[],
            },
            status: String::new(),
            mode: Mode::Browse,
            expanded: false,
            quit: false,
        };
        app.reload()?;
        Ok(app)
    }

    /// Full field text for the selected row, when the detail pane is open.
    pub(crate) fn selected_detail(&self) -> Option<&[(&'static str, String)]> {
        if !self.expanded {
            return None;
        }
        self.table.details.get(self.selected).map(Vec::as_slice)
    }

    pub(crate) fn run<B>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        while !self.quit {
            terminal
                .draw(|frame| ui::render(frame, self))
                .map_err(|error| goal_tracker_core::Error::Io(std::io::Error::other(error)))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        if event::poll(POLL_INTERVAL)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            self.on_key(key.code)?;
        }
        Ok(())
    }

    fn on_key(&mut self, code: KeyCode) -> Result<()> {
        match self.mode {
            Mode::Browse => self.browse_key(code),
            Mode::Form(_) => self.form_key(code),
        }
    }

    fn browse_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc if self.expanded => self.expanded = false,
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Tab => self.cycle_section(true)?,
            KeyCode::BackTab => self.cycle_section(false)?,
            KeyCode::Right => self.expanded = true,
            KeyCode::Left => self.expanded = false,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(Direction::Next),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(Direction::Previous),
            KeyCode::Char('r') => self.reload()?,
            KeyCode::Char('a' | 'i') => self.open_form(false)?,
            KeyCode::Char('e') | KeyCode::Enter => self.open_form(true)?,
            _ => {}
        }
        Ok(())
    }

    fn form_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Browse;
                return Ok(());
            }
            KeyCode::Enter => return self.submit_form(),
            _ => {}
        }
        if let Mode::Form(form) = &mut self.mode {
            let selected = form.selected;
            match code {
                KeyCode::Tab | KeyCode::Down => form.move_field(Direction::Next),
                KeyCode::BackTab | KeyCode::Up => form.move_field(Direction::Previous),
                KeyCode::Backspace => {
                    form.fields[selected].value.pop();
                }
                KeyCode::Char(character) => form.fields[selected].value.push(character),
                _ => {}
            }
            form.error = None;
        }
        Ok(())
    }

    fn open_form(&mut self, edit: bool) -> Result<()> {
        let id = if edit {
            self.table.ids.get(self.selected).cloned()
        } else {
            None
        };
        if edit && id.is_none() {
            return Ok(());
        }
        let fields = self
            .section
            .form_fields(self.database.connection(), id.as_deref())?;
        let verb = if edit { "Edit" } else { "Add" };
        self.mode = Mode::Form(Form {
            title: format!("{verb} {}", self.section.noun()),
            fields,
            selected: 0,
            id,
            error: None,
        });
        Ok(())
    }

    fn submit_form(&mut self) -> Result<()> {
        let Mode::Form(form) = &self.mode else {
            return Ok(());
        };
        let values: Vec<String> = form
            .fields
            .iter()
            .map(|field| field.value.trim().to_owned())
            .collect();
        let id = form.id.clone();
        let result = self
            .section
            .save(&mut self.database, id.as_deref(), &values);
        match result {
            Ok(()) => {
                self.mode = Mode::Browse;
                self.reload()
            }
            Err(error) => {
                if let Mode::Form(form) = &mut self.mode {
                    form.error = Some(error.to_string());
                }
                Ok(())
            }
        }
    }

    fn cycle_section(&mut self, forward: bool) -> Result<()> {
        let count = Section::ALL.len();
        let index = self.section.index();
        let next = if forward {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        self.section = Section::ALL[next];
        self.selected = 0;
        self.expanded = false;
        self.reload()
    }

    fn move_selection(&mut self, direction: Direction) {
        if self.table.rows.is_empty() {
            self.selected = 0;
            return;
        }
        let last = self.table.rows.len() - 1;
        self.selected = match direction {
            Direction::Next => (self.selected + 1).min(last),
            Direction::Previous => self.selected.saturating_sub(1),
        };
    }

    fn reload(&mut self) -> Result<()> {
        let table = self.section.load(self.database.connection())?;
        if self.selected >= table.rows.len() {
            self.selected = 0;
        }
        self.status = format!("{} · {} rows", self.section.title(), table.rows.len());
        self.table = table;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{level_bar, table};
    use crate::form;
    use goal_tracker_core::model;
    use goal_tracker_core::store::Database;

    #[test]
    fn level_bar_fills_proportionally() {
        assert_eq!(level_bar(0), "░░░░░░░░░░ 0/10");
        assert_eq!(level_bar(6), "██████░░░░ 6/10");
        assert_eq!(level_bar(10), "██████████ 10/10");
    }

    #[test]
    fn level_bar_clamps_out_of_range_levels() {
        assert_eq!(level_bar(99), "██████████ 99/10");
        assert_eq!(level_bar(-1), "░░░░░░░░░░ -1/10");
    }

    #[test]
    fn table_carries_full_field_text_per_record() {
        let mut database = Database::open_in_memory().expect("database");
        let values = vec![
            "Systems work".to_owned(),
            "A description that is far too long for one table column".to_owned(),
            "active".to_owned(),
        ];
        form::save::<model::Goal>(&mut database, None, &values).expect("save");

        let table = table::<model::Goal>(database.connection()).expect("table");
        assert_eq!(table.details.len(), 1);
        let detail = &table.details[0];
        assert!(detail.iter().any(|(label, value)| {
            *label == "Description"
                && value == "A description that is far too long for one table column"
        }));
    }
}
