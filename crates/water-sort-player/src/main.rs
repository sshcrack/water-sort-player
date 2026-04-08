mod app;
mod app_visualization;
#[cfg(test)]
mod level_tests;

pub use water_sort_capture as capture;
pub use water_sort_core::{bottles, constants, position};
pub use water_sort_solver as solver;

fn main() {
    let quick_mode = std::env::args().any(|arg| arg == "--quick");

    if let Err(error) = app::run(quick_mode) {
        eprintln!("Error: {}", error);
    }
}
