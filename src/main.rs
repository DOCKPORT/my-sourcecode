mod app;
mod scan;
mod store;
mod ui;

use crate::app::App;
use crate::ui::tui;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Terminal analyzer for the source code of a local git repository.
#[derive(Parser)]
#[command(
    name = "my-sourcecode",
    version,
    about = "Analyze the source code of a local git repository"
)]
struct Args {
    /// Path to a local git repository to scan directly, skipping the picker.
    #[arg(value_name = "REPOSITORY")]
    repository: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut app = App::new();
    if let Some(path) = &args.repository {
        app.open_repository(path);
    }

    let terminal = tui::init()?;
    app.run(terminal)
}
