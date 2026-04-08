use std::sync::{Once, atomic::AtomicBool};

use crate::scrcpy::emergency_cleanup;


pub static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);
pub static SIGNAL_HANDLER_ONCE: Once = Once::new();


pub fn install_signal_handler() {
    SIGNAL_HANDLER_ONCE.call_once(|| {
        if let Err(error) = ctrlc::set_handler(|| {
            eprintln!("Ctrl+C received, cleaning up...");
            emergency_cleanup();
            std::process::exit(130);
        }) {
            eprintln!("Failed to install Ctrl+C handler: {error}");
        }
    });
}