mod app;
mod audio;
mod config;
mod error;
mod library;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use crate::app::App;

#[derive(Debug, Parser)]
#[command(
    name = "sonora",
    version,
    about = "Audiophile FLAC terminal music player"
)]
struct Cli {
    path: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut app = App::new(cli.path)?;

    let mut terminal = ratatui::try_init()?;
    let run_result = app.run(&mut terminal);
    let restore_result = ratatui::try_restore();

    run_result?;
    restore_result?;
    Ok(())
}
