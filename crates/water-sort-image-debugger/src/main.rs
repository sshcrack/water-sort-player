use std::{
    collections::HashSet, fs, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}
};

use anyhow::{Result, anyhow, bail};
use clap::Parser;
use log::{error, info};
use opencv::{
    core::{Mat, MatTraitConst},
    imgcodecs,
};
use water_sort_core::detect_bottles;

#[derive(Debug, Parser)]
#[command(
    name = "water-sort-image-debugger",
    about = "Detects layout and bottles from an input image and writes an annotated frame"
)]
struct CliArgs {
    #[arg(short, long, value_name = "IMAGE_PATH")]
    input: Option<PathBuf>,

    #[arg(short = 'o', long = "out", value_name = "OUT_PATH")]
    output: Option<PathBuf>,

    #[arg(long, help = "Print available layout names and exit")]
    list_layouts: bool,
}

fn default_output_path() -> Result<PathBuf> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(PathBuf::from(format!(
        "target/frame_display-{timestamp}.png"
    )))
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn load_image(input_path: &Path) -> Result<Mat> {
    let frame_raw = imgcodecs::imread(
        input_path.to_string_lossy().as_ref(),
        imgcodecs::IMREAD_COLOR,
    )?;

    if frame_raw.empty() {
        bail!("failed to load image at {}", input_path.display());
    }

    Ok(frame_raw)
}

fn save_image(path: &Path, image: &Mat) -> Result<()> {
    ensure_parent_dir(path)?;
    let ok = imgcodecs::imwrite(
        path.to_string_lossy().as_ref(),
        image,
        &opencv::core::Vector::new(),
    )?;

    if !ok {
        bail!("failed to write output image to {}", path.display());
    }

    Ok(())
}

fn run() -> Result<()> {
    let args = CliArgs::parse();

    let input_path = args
        .input
        .ok_or_else(|| anyhow!("missing input image path, use --input <path>"))?;

    let frame_raw = load_image(&input_path)?;
    let mut frame_display = frame_raw.try_clone()?;

    let bottles = detect_bottles(&frame_raw, &mut frame_display, &mut HashSet::new())?;
    info!(
        "Detected bottles: {}",
        bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let output_path = match args.output {
        Some(path) => path,
        None => default_output_path()?,
    };

    save_image(&output_path, &frame_display)?;
    info!("Saved frame display to {}", output_path.display());

    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    if let Err(error) = run() {
        error!("Error: {error:#}");
        std::process::exit(1);
    }
}
