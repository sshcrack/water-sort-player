use std::{
    path::PathBuf,
    process::{Child, ChildStdout, Command, Stdio},
    sync::Mutex,
};

use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use water_sort_core::Pos;

use crate::VIRTUAL_CAM;

lazy_static! {
    static ref COMPUTER_TO_MOBILE_SCALE: Mutex<(f32, f32)> = Mutex::new((1.0, 1.0));
}

pub struct ScrcpyChild(Child);

impl ScrcpyChild {
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.0.stdout.take()
    }
}

impl Drop for ScrcpyChild {
    fn drop(&mut self) {
        if let Err(error) = self.0.kill() {
            eprintln!("Failed to kill scrcpy process: {}", error);
        }
    }
}

pub fn get_adb_path() -> PathBuf {
    let current_executable = std::env::current_exe().unwrap();
    let current_dir = current_executable.parent().unwrap();
    let adb_path = current_dir.join("adb");

    if !adb_path.exists() {
        panic!("adb executable not found at: {}", adb_path.display());
    }

    adb_path
}

pub fn click_at_position(pos: Pos) {
    click_at(pos.0, pos.1);
}

pub fn click_at(x: i32, y: i32) {
    let (scale_x, scale_y) = *COMPUTER_TO_MOBILE_SCALE.lock().unwrap();
    let x = (x as f32 * scale_x) as i32;
    let y = (y as f32 * scale_y) as i32;

    Command::new(get_adb_path())
        .args(["shell", "input", "tap", &x.to_string(), &y.to_string()])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

pub fn start_scrcpy(quick_mode: bool) -> Result<ScrcpyChild> {
    let current_executable = std::env::current_exe()?;
    let current_dir = current_executable
        .parent()
        .ok_or_else(|| anyhow!("failed to get parent directory of executable"))?;

    let scrcpy_path = current_dir.join("scrcpy");
    if !scrcpy_path.exists() {
        return Err(anyhow!(
            "scrcpy executable not found at: {}",
            scrcpy_path.display()
        ));
    }
    let mut cmd = Command::new("stdbuf");
    cmd.args([
        "-oL",
        scrcpy_path.to_str().unwrap(),
        "--stay-awake",
        "--no-audio",
        "--mouse=disabled",
        "--keyboard=disabled",
        "--gamepad=disabled",
        "--max-size=800",
        "--max-fps=15",
        "--video-bit-rate=2M",
        "--video-codec=h264",
        "--no-clipboard-autosync",
        "--window-title=AutoPlayer",
        "--no-video-playback",
        format!("--v4l2-sink={}", VIRTUAL_CAM).as_str(),
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    if !quick_mode {
        cmd.arg("--start-app=+com.no1ornothing.color.water.sort.woody.puzzle");
    }

    let child = cmd.spawn()?;

    Ok(ScrcpyChild(child))
}

pub fn measure_window_to_mobile_scale(width: usize, height: usize) {
    let size = Command::new(get_adb_path())
        .args(["shell", "wm", "size"])
        .output()
        .unwrap();

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

    let scale_x = mobile_width / width as f32;
    let scale_y = mobile_height / height as f32;

    let mut scale_lock = COMPUTER_TO_MOBILE_SCALE.lock().unwrap();
    *scale_lock = (scale_x, scale_y);

    println!("Computed scale factors - X: {}, Y: {}", scale_x, scale_y);
}