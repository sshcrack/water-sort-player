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
use serde_json::{Value, json};

use water_sort_core::{
    bottles::{Bottle, BottleLayout},
    constants::BottleColor,
};

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
const DISCOVERY_MANIFEST_PATH: &str = "captures/discovery_levels.json";

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct DiscoveryLevelCapture {
    pub id: u64,
    pub image_filename: String,
    pub expected_bottles: String,
    pub resolved_bottles: Option<String>,
    pub layout_name: String,
    pub bottle_count: usize,
    pub mystery_count_at_start: usize,
    pub captured_at_ms: u64,
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
#[derive(Debug, Clone, Default)]
pub struct DiscoveryCaptureManifest {
    pub levels: Vec<DiscoveryLevelCapture>,
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
#[derive(Debug, Clone)]
pub struct DiscoveryCaptureContext {
    pub level: DiscoveryLevelCapture,
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
impl DiscoveryCaptureContext {
    pub fn finalize(self) -> Result<()> {
        upsert_discovery_capture(self.level)
    }

    pub fn set_resolved_bottles(&mut self, bottles: &[Bottle]) {
        self.level.resolved_bottles = Some(bottles_to_sequence(bottles));
    }
}

fn rgba_frame_from_bgr(frame: &Mat) -> Result<Mat> {
    let mut frame_rgba = Mat::default();
    imgproc::cvt_color(frame, &mut frame_rgba, imgproc::COLOR_BGR2RGBA, 0)?;
    Ok(frame_rgba)
}

fn current_time_ms() -> Result<u64> {
    Ok(u64::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
    )?)
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn next_discovery_capture_id() -> Result<u64> {
    let manifest = read_discovery_manifest()?;
    let next_id = manifest
        .levels
        .iter()
        .map(|entry| entry.id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    Ok(next_id)
}

fn save_png_with_filename(frame: &Mat, filename: &str) -> Result<PathBuf> {
    let frame_rgba = rgba_frame_from_bgr(frame)?;
    let bytes = frame_rgba.data_bytes()?;
    let size = frame_rgba.size()?;

    let width = u32::try_from(size.width).map_err(|_| anyhow!("invalid frame width"))?;
    let height = u32::try_from(size.height).map_err(|_| anyhow!("invalid frame height"))?;

    fs::create_dir_all("captures")?;

    let path = PathBuf::from(format!("captures/{filename}"));

    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, bytes.to_vec())
        .ok_or_else(|| anyhow!("failed to build PNG buffer"))?;
    image.save(&path)?;

    Ok(path)
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
pub fn bottles_to_sequence(bottles: &[Bottle]) -> String {
    bottles
        .iter()
        .map(bottle_to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn bottle_to_string(bottle: &Bottle) -> String {
    // Bottle fills are stored bottom->top. Test strings are top->bottom with 'E' for empty slots.
    let fills = bottle.get_fills();
    let mut slots = [BottleColor::Mystery; 4];
    for (index, color) in fills.iter().enumerate().take(4) {
        slots[index] = *color;
    }

    let mut out = String::with_capacity(4);
    for slot in (0..4).rev() {
        if slot >= fills.len() {
            out.push('E');
            continue;
        }

        out.push(color_to_char(slots[slot]));
    }
    out
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn color_to_char(color: BottleColor) -> char {
    match color {
        BottleColor::Yellow => 'Y',
        BottleColor::Red => 'R',
        BottleColor::Green => 'G',
        BottleColor::Lime => 'g',
        BottleColor::LightBlue => 'L',
        BottleColor::MediumBlue => 'M',
        BottleColor::Blue => 'B',
        BottleColor::Purple => 'P',
        BottleColor::Orange => 'O',
        BottleColor::Pink => 'W',
        BottleColor::Mystery => '?',
    }
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn count_mystery_colors(bottles: &[Bottle]) -> usize {
    bottles
        .iter()
        .map(|bottle| {
            bottle
                .get_fills()
                .iter()
                .filter(|color| **color == BottleColor::Mystery)
                .count()
        })
        .sum()
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn read_discovery_manifest() -> Result<DiscoveryCaptureManifest> {
    match fs::read_to_string(DISCOVERY_MANIFEST_PATH) {
        Ok(content) => {
            let parsed = serde_json::from_str::<Value>(&content)?;
            Ok(manifest_from_json(&parsed))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(DiscoveryCaptureManifest::default())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn write_discovery_manifest(manifest: &DiscoveryCaptureManifest) -> Result<()> {
    fs::create_dir_all("captures")?;
    let manifest_json = manifest_to_json(manifest);
    let manifest_text = serde_json::to_string_pretty(&manifest_json)?;
    fs::write(DISCOVERY_MANIFEST_PATH, manifest_text)?;
    Ok(())
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn manifest_to_json(manifest: &DiscoveryCaptureManifest) -> Value {
    json!({
        "levels": manifest
            .levels
            .iter()
            .map(level_to_json)
            .collect::<Vec<_>>()
    })
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn level_to_json(level: &DiscoveryLevelCapture) -> Value {
    json!({
        "id": level.id,
        "image_filename": level.image_filename,
        "expected_bottles": level.expected_bottles,
        "resolved_bottles": level.resolved_bottles,
        "layout_name": level.layout_name,
        "bottle_count": level.bottle_count,
        "mystery_count_at_start": level.mystery_count_at_start,
        "captured_at_ms": level.captured_at_ms,
    })
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn manifest_from_json(value: &Value) -> DiscoveryCaptureManifest {
    let levels = value
        .get("levels")
        .and_then(|levels| levels.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(level_from_json)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    DiscoveryCaptureManifest { levels }
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn level_from_json(value: &Value) -> Option<DiscoveryLevelCapture> {
    Some(DiscoveryLevelCapture {
        id: value.get("id")?.as_u64()?,
        image_filename: value.get("image_filename")?.as_str()?.to_string(),
        expected_bottles: value.get("expected_bottles")?.as_str()?.to_string(),
        resolved_bottles: value
            .get("resolved_bottles")
            .and_then(Value::as_str)
            .map(str::to_string),
        layout_name: value
            .get("layout_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        bottle_count: value
            .get("bottle_count")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        mystery_count_at_start: value
            .get("mystery_count_at_start")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        captured_at_ms: value
            .get("captured_at_ms")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    })
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
fn upsert_discovery_capture(capture: DiscoveryLevelCapture) -> Result<()> {
    let mut manifest = read_discovery_manifest()?;

    if let Some(existing) = manifest
        .levels
        .iter_mut()
        .find(|entry| entry.expected_bottles == capture.expected_bottles)
    {
        *existing = capture;
    } else {
        manifest.levels.push(capture);
    }

    manifest.levels.sort_by_key(|entry| entry.id);
    write_discovery_manifest(&manifest)
}

#[cfg_attr(not(feature = "collect-test-data"), allow(dead_code))]
pub fn start_discovery_capture(
    frame: &Mat,
    layout: &BottleLayout,
    bottles: &[Bottle],
) -> Result<DiscoveryCaptureContext> {
    let capture_id = next_discovery_capture_id()?;
    let captured_at_ms = current_time_ms()?;
    let image_filename = format!("discovery-level-{capture_id}.png");
    save_png_with_filename(frame, &image_filename)?;

    let mystery_count = count_mystery_colors(bottles);
    Ok(DiscoveryCaptureContext {
        level: DiscoveryLevelCapture {
            id: capture_id,
            image_filename,
            expected_bottles: bottles_to_sequence(bottles),
            resolved_bottles: None,
            layout_name: layout.name.clone(),
            bottle_count: layout.bottle_count(),
            mystery_count_at_start: mystery_count,
            captured_at_ms,
        },
    })
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
    let timestamp = current_time_ms()?;
    save_png_with_filename(frame, &format!("frame-{timestamp}.png"))
}
