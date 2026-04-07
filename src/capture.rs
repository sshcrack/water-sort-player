use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow};
use image::{ImageBuffer, Rgba};
use opencv::{
    core::{Mat, MatTraitConst, MatTraitConstManual},
    imgproc,
};

fn rgba_frame_from_bgr(frame: &Mat) -> Result<Mat> {
    let mut frame_rgba = Mat::default();
    imgproc::cvt_color(frame, &mut frame_rgba, imgproc::COLOR_BGR2RGBA, 0)?;
    Ok(frame_rgba)
}

pub fn frame_to_window_buffer(frame: &Mat) -> Result<Vec<u32>> {
    let frame_rgba = rgba_frame_from_bgr(frame)?;
    let bytes = frame_rgba.data_bytes()?;

    Ok(bytes
        .chunks_exact(4)
        .map(|pixel| ((pixel[0] as u32) << 16) | ((pixel[1] as u32) << 8) | (pixel[2] as u32))
        .collect())
}

pub fn save_frame_png(frame: &Mat) -> Result<PathBuf> {
    let frame_rgba = rgba_frame_from_bgr(frame)?;
    let bytes = frame_rgba.data_bytes()?;
    let size = frame_rgba.size()?;

    let width = u32::try_from(size.width).map_err(|_| anyhow!("invalid frame width"))?;
    let height = u32::try_from(size.height).map_err(|_| anyhow!("invalid frame height"))?;

    fs::create_dir_all("captures")?;

    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let path = PathBuf::from(format!("captures/frame-{timestamp}.png"));

    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, bytes.to_vec())
        .ok_or_else(|| anyhow!("failed to build PNG buffer"))?;
    image.save(&path)?;

    Ok(path)
}
