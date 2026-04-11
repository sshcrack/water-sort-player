#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

use anyhow::Result;
use opencv::core::Mat;
use water_sort_core::Pos;

pub trait CaptureDeviceBackend {
    fn start_capture(&mut self, quick_mode: bool) -> Result<(usize, usize)>;

    fn capture_frame(&mut self) -> Result<Mat>;

    fn click_at_position(&self, pos: Pos) -> Result<()> {
        self.click_at(pos.0, pos.1)
    }

    fn click_at(&self, x: i32, y: i32) -> Result<()>;
}

pub fn construct_capture_backend() -> impl CaptureDeviceBackend {
    #[cfg(target_os = "linux")]
    {
        linux::ScrcpyVirtualCamBackend::new()
    }
    #[cfg(not(target_os = "linux"))]
    {
        windows::ScrcpyRecordBackend::new()
    }
}
