use std::{
    env,
    path::PathBuf,
    process::{Child, Command},
};

use anyhow::{Context, anyhow};
use log::{error, info};

pub struct ScrcpyChild(pub(crate) Child);

impl ScrcpyChild {
    #[cfg(target_os = "linux")]
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.0.stdout.take()
    }
}

impl Drop for ScrcpyChild {
    fn drop(&mut self) {
        if let Err(error) = self.0.kill() {
            error!("Failed to kill scrcpy process: {}", error);
        }
    }
}

pub(crate) fn get_adb_path() -> PathBuf {
    let current_executable = env::current_exe().unwrap();
    let current_dir = current_executable.parent().unwrap();
    let exe_file = if cfg!(target_os = "windows") {
        "adb.exe"
    } else {
        "adb"
    };

    let adb_path = current_dir.join(exe_file);
    if !adb_path.exists() {
        panic!("adb executable not found at: {}", adb_path.display());
    }

    adb_path
}

pub(crate) fn measure_window_to_mobile_scale(
    width: usize,
    height: usize,
) -> anyhow::Result<(f32, f32)> {
    let size = Command::new(get_adb_path())
        .args(["shell", "wm", "size"])
        .output()
        .context("failed to query device screen size over adb")?;

    let output = String::from_utf8_lossy(&size.stdout);
    let mut mobile_width = 0.0;
    let mut mobile_height = 0.0;

    for line in output.lines() {
        if line.contains("Physical size:") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                let dims: Vec<&str> = parts[1].trim().split('x').collect();
                if dims.len() == 2 {
                    mobile_width = dims[0].parse::<f32>().unwrap_or(0.0);
                    mobile_height = dims[1].parse::<f32>().unwrap_or(0.0);
                }
            }
        }
    }

    if mobile_width <= 0.0 || mobile_height <= 0.0 {
        return Err(anyhow!(
            "unable to parse device physical size from adb output"
        ));
    }

    let scale_x = mobile_width / width as f32;
    let scale_y = mobile_height / height as f32;

    info!("Computed scale factors - X: {}, Y: {}", scale_x, scale_y);
    Ok((scale_x, scale_y))
}
