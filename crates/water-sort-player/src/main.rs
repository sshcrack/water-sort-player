mod app;
mod app_visualization;
#[cfg(test)]
mod level_tests;

use clap::Parser;
use log::error;
use std::path::PathBuf;

pub use water_sort_capture as capture;
pub use water_sort_core::{bottles, constants, position};
pub use water_sort_solver as solver;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[arg(long)]
    quick: bool,

    #[arg(long = "use-state", value_name = "PATH")]
    use_state: Option<PathBuf>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse();

    if let Err(error) = app::run(cli.quick, cli.use_state.as_deref()) {
        error!("Error: {}", error);
    }
}
