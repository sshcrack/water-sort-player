mod app;
mod bottles;
mod capture;
mod constants;
mod position;
mod scrcpy;

fn main() {
    if let Err(error) = app::run() {
        eprintln!("Error: {}", error);
    }
}
