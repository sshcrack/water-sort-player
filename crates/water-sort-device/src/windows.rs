use std::{io::Write, sync::Mutex, time::Duration};

use anyhow::{Result, anyhow};
use opencv::core::{Mat, MatTraitConst};
use win_screenshot::{
    prelude::{Using, capture_window_ex},
    utils::find_window,
};

use crate::{
    CaptureDeviceBackend,
    common::{ScrcpyChild, measure_window_to_mobile_scale},
};

pub struct ScrcpyRecordBackend {
    scrcpy_child: Option<ScrcpyChild>,
    scrcpy_window: Option<isize>,
    scale: Mutex<(f32, f32)>,
}

impl Default for ScrcpyRecordBackend {
    fn default() -> Self {
        Self {
            scrcpy_child: None,
            scrcpy_window: None,
            scale: Mutex::new((1.0, 1.0)),
        }
    }
}

impl ScrcpyRecordBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

fn spawn_scrcpy(quick_mode: bool) -> Result<ScrcpyChild> {
    let mut cmd = std::process::Command::new("scrcpy");
    cmd.arg("--max-size")
        .arg("800")
        .arg("--max-fps")
        .arg("15")
        .arg("--stay-awake")
        .arg("--no-audio")
        .arg("--mouse=disabled")
        .arg("--keyboard=disabled")
        .arg("--gamepad=disabled")
        .arg("--no-clipboard-autosync")
        .arg("--window-title=WaterSortPlayer");
    if !quick_mode {
        cmd.arg("--start-app=+com.no1ornothing.color.water.sort.woody.puzzle");
    }
    let child = cmd
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn scrcpy: {}", e))?;
    Ok(ScrcpyChild(child))
}

impl CaptureDeviceBackend for ScrcpyRecordBackend {
    fn start_capture(&mut self, quick_mode: bool) -> Result<(usize, usize)> {
        let child = spawn_scrcpy(quick_mode)?;

        self.scrcpy_child = Some(child);
        print!("Waiting for scrcpy window to appear..");
        loop {
            print!(".");
            std::io::stdout().flush().unwrap();
            let window = find_window("WaterSortPlayer");
            if let Ok(window) = window {
                self.scrcpy_window = Some(window);
                break;
            }

            std::thread::sleep(Duration::from_millis(500));
        }
        std::thread::sleep(Duration::from_millis(2000));

        let hwnd = *self.scrcpy_window.as_ref().unwrap();
        let capture = capture_window_ex(
            hwnd,
            Using::BitBlt,
            win_screenshot::prelude::Area::ClientOnly,
            None,
            None,
        )?;

        let width = capture.width as usize;
        let height = capture.height as usize;
        println!(
            "Found scrcpy window with dimensions: {}x{}",
            capture.width, capture.height
        );

        {
            let mut scale_lock = self.scale.lock().unwrap();
            *scale_lock = measure_window_to_mobile_scale(width, height)?;
        }

        println!("\nFound scrcpy window!");
        Ok((width, height))
    }

    fn capture_frame(&mut self) -> anyhow::Result<Mat> {
        let hwnd = self
            .scrcpy_window
            .as_ref()
            .ok_or_else(|| anyhow!("scrcpy window not found"))?;

        let capture = capture_window_ex(
            *hwnd,
            Using::PrintWindow,
            win_screenshot::prelude::Area::ClientOnly,
            None,
            None,
        )?;
        let height = capture.height;

        let bgr_data: Vec<u8> = capture
            .pixels
            .chunks(4)
            .flat_map(|p| [p[2], p[1], p[0]])
            .collect();

        let mat = Mat::from_slice(&bgr_data)?;
        let mat = mat.reshape(3, height as i32)?;

        Ok(mat.try_clone()?)
    }

    fn get_scale(&self) -> (f32, f32) {
        *self.scale.lock().unwrap()
    }
}
