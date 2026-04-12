#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

mod common;

use std::process::Command;

use anyhow::{Context, Result, anyhow};
use opencv::core::Mat;
use water_sort_core::Pos;

use crate::common::get_adb_path;

pub trait CaptureDeviceBackend {
    fn start_capture(&mut self, quick_mode: bool) -> Result<(usize, usize)>;

    fn capture_frame(&mut self) -> Result<Mat>;

    fn click_at_position(&self, pos: Pos) -> Result<()> {
        self.click_at(pos.0, pos.1)
    }

    fn get_scale(&self) -> (f32, f32);

    fn restart_app(&self) -> Result<()> {
        let status = Command::new(get_adb_path())
            .args([
                "shell",
                "am",
                "force-stop",
                "com.no1ornothing.color.water.sort.woody.puzzle",
            ])
            .status()
            .context("failed to execute adb force-stop command")?;

        if !status.success() {
            return Err(anyhow!(
                "adb force-stop command exited with status: {}",
                status
            ));
        }

        let status = Command::new(get_adb_path())
            .args([
                "shell",
                "monkey",
                "-p",
                "com.no1ornothing.color.water.sort.woody.puzzle",
                "-c",
                "android.intent.category.LAUNCHER",
                "1",
            ])
            .status()
            .context("failed to execute adb start command")?;

        if !status.success() {
            return Err(anyhow!("adb start command exited with status: {}", status));
        }

        Ok(())
    }

    fn click_at(&self, x: i32, y: i32) -> Result<()> {
        let (scale_x, scale_y) = self.get_scale();
        let x = (x as f32 * scale_x) as i32;
        let y = (y as f32 * scale_y) as i32;

        let status = Command::new(get_adb_path())
            .args(["shell", "input", "tap", &x.to_string(), &y.to_string()])
            .status()
            .context("failed to execute adb tap command")?;

        if !status.success() {
            return Err(anyhow!("adb tap command exited with status: {}", status));
        }

        Ok(())
    }
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
