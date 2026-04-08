mod app;
mod app_visualization;
mod bottles;
mod capture;
mod constants;
#[cfg(feature = "fps-measure")]
mod fps_measure;
#[cfg(test)]
mod level_tests;
mod position;
mod scrcpy;
mod solver;
pub mod shutdown;

fn main() {
    let quick_mode = std::env::args().any(|arg| arg == "--quick");

    #[cfg(feature = "fps-measure")]
    if let Err(error) = fps_measure::run(quick_mode) {
        eprintln!("Error: {}", error);
    }

    #[cfg(not(feature = "fps-measure"))]
    if let Err(error) = app::run(quick_mode) {
        eprintln!("Error: {}", error);
    }
}
