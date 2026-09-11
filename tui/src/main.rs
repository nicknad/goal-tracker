mod app;
mod cli;
mod form;
mod ui;

use clap::Parser;
use goal_tracker_core::{Database, Result};

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let mut database = Database::open(&cli.db)?;
    match cli.command {
        Some(command) => cli::run(&mut database, command),
        None => run_tui(database),
    }
}

fn run_tui(database: Database) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = app::App::new(database).and_then(|mut app| app.run(&mut terminal));
    ratatui::restore();
    result
}
