use std::sync::Mutex;

use anyhow::{Result, anyhow};
use opencv::core::Mat;

use crate::DeviceBackend;

pub struct ScrcpyRecordBackend {
    taps: Mutex<Vec<(i32, i32)>>,
}

impl Default for ScrcpyRecordBackend {
    fn default() -> Self {
        Self {
            taps: Mutex::new(Vec::new()),
        }
    }
}

impl ScrcpyRecordBackend {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn taps(&self) -> Vec<(i32, i32)> {
        self.taps.lock().unwrap().clone()
    }
}

impl DeviceBackend for MockBackend {
    fn start_capture(&mut self, _quick_mode: bool) -> Result<(usize, usize)> {
        Err(anyhow!(
            "MockBackend::start_capture is a scaffold and does not provide frames yet"
        ))
    }

    fn capture_frame(&mut self) -> Result<Mat> {
        Err(anyhow!(
            "MockBackend::capture_frame is a scaffold and does not provide frames yet"
        ))
    }

    fn click_at(&self, x: i32, y: i32) -> Result<()> {
        self.taps.lock().unwrap().push((x, y));
        Ok(())
    }
}
