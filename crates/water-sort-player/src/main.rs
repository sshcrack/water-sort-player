mod app;
mod app_visualization;
#[cfg(test)]
mod level_tests;

use log::error;

pub use water_sort_capture as capture;
pub use water_sort_core::{bottles, constants, position};
pub use water_sort_solver as solver;

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .format_timestamp_millis()
    .init();

    let quick_mode = std::env::args().any(|arg| arg == "--quick");

    if let Err(error) = app::run(quick_mode) {
        error!("Error: {}", error);
    }
}
