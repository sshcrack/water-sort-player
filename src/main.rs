mod app;
mod bottles;
mod capture;
mod constants;
mod position;
mod scrcpy;

fn main() {
    let quick_mode = std::env::args().any(|arg| arg == "--quick");

    if let Err(error) = app::run(quick_mode) {
        eprintln!("Error: {}", error);
    }
}
