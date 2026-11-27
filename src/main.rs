mod app;
mod audio;
mod config;
mod error;
mod library;
mod ui;

use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

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

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let _ = ratatui::try_restore();
            write_error_log(&err.to_string());
            eprintln!("Sonora failed to start: {err:#}");
            if io::stdin().is_terminal() {
                eprintln!("\nPress Enter to exit...");
                let mut line = String::new();
                let _ = io::stdin().read_line(&mut line);
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut app = App::new(cli.path);

    let mut terminal = ratatui::try_init()?;
    let run_result = app.run(&mut terminal);
    let restore_result = ratatui::try_restore();

    run_result?;
    restore_result?;
    Ok(())
}

fn write_error_log(message: &str) {
    let Some(dir) = dirs::config_dir().map(|base| base.join("sonora")) else {
        return;
    };
    if fs::create_dir_all(&dir).is_ok() {
        let _ = fs::write(dir.join("last_error.log"), message);
    }
}
