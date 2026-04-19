use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow};
use opencv::{
    core::{Mat, Vector},
    imgcodecs,
};
use serde_json::json;

use super::AppState;

#[derive(Debug, Clone)]
pub(super) struct SaveStatesRecorder {
    output_root: PathBuf,
    current_level_id: u64,
    transition_index: u64,
}

impl SaveStatesRecorder {
    pub(super) fn new() -> Result<Self> {
        let current_exe = std::env::current_exe()?;
        let exe_dir = current_exe
            .parent()
            .ok_or_else(|| anyhow!("failed to determine executable directory"))?;
        let output_root = exe_dir
            .join("save-states")
            .join(current_time_ms()?.to_string());
        fs::create_dir_all(&output_root)?;

        Ok(Self {
            output_root,
            current_level_id: 1,
            transition_index: 0,
        })
    }

    pub(super) fn capture_transition(
        &mut self,
        previous_state: &AppState,
        app_state: &AppState,
        frame_raw: &Mat,
        frame_display: &Mat,
    ) -> Result<()> {
        if matches!(previous_state, AppState::ClickNextLevel { .. }) {
            self.current_level_id = self.current_level_id.saturating_add(1);
            self.transition_index = 0;
        }

        self.transition_index = self.transition_index.saturating_add(1);

        let level_dir = self
            .output_root
            .join(format!("level-{:04}", self.current_level_id));
        fs::create_dir_all(&level_dir)?;

        log::trace!(
            "Saving transition {} for level {}",
            self.transition_index,
            self.current_level_id
        );
        let file_stem = format!("{:04}-{}", self.transition_index, app_state.get_name());
        let raw_image_filename = format!("{file_stem}.png");
        let ui_image_filename = format!("{file_stem}-ui.png");
        let image_path = level_dir.join(&raw_image_filename);
        let ui_image_path = level_dir.join(&ui_image_filename);
        let json_path = level_dir.join(format!("{file_stem}.json"));

        save_frame_to_path(frame_raw, &image_path)?;
        save_frame_to_path(frame_display, &ui_image_path)?;

        let payload = json!({
            "captured_at_ms": current_time_ms()?,
            "level_id": self.current_level_id,
            "transition_index": self.transition_index,
            "raw_image_filename": raw_image_filename,
            "ui_image_filename": ui_image_filename,
            "previous_state": previous_state,
            "app_state": app_state,
        });

        fs::write(json_path, serde_json::to_string_pretty(&payload)?)?;

        Ok(())
    }
}

fn save_frame_to_path(frame: &Mat, path: &Path) -> Result<()> {
    let path_display = path
        .to_str()
        .ok_or_else(|| anyhow!("state image output path is not valid UTF-8"))?;
    let ok = imgcodecs::imwrite(path_display, frame, &Vector::new())?;
    if !ok {
        return Err(anyhow!(
            "OpenCV failed to write state image to {}",
            path.display()
        ));
    }
    Ok(())
}

fn current_time_ms() -> Result<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}
