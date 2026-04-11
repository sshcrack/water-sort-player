use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Result, anyhow, bail};
use clap::Parser;
use opencv::{
    core::{Mat, MatTraitConst},
    imgcodecs,
};
use water_sort_capture::bottles_to_sequence;
use water_sort_core::{BottleLayout, detect_bottles_with_layout};

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

    #[arg(short = 'l', long = "layout", value_name = "LAYOUT_NAME")]
    layout_name: Option<String>,

    #[arg(long, help = "Print available layout names and exit")]
    list_layouts: bool,
}

fn find_layout_by_name(name: &str) -> Result<BottleLayout> {
    let normalized = name.to_ascii_lowercase();
    BottleLayout::get_layouts()
        .into_iter()
        .find(|layout| {
            layout.name.eq_ignore_ascii_case(name)
                || layout.name.to_ascii_lowercase() == normalized
        })
        .ok_or_else(|| {
            let available = BottleLayout::get_layouts()
                .into_iter()
                .map(|layout| layout.name)
                .collect::<Vec<_>>()
                .join(", ");
            anyhow!("unknown layout '{name}'. Available layouts: {available}")
        })
}

fn default_output_path() -> Result<PathBuf> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    Ok(PathBuf::from(format!(
        "captures/frame_display-{timestamp}.png"
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

    if args.list_layouts {
        println!("Available layouts:");
        for layout in BottleLayout::get_layouts() {
            println!("  - {}", layout.name);
        }

        if args.input.is_none() {
            return Ok(());
        }
    }

    let input_path = args
        .input
        .ok_or_else(|| anyhow!("missing input image path, use --input <path>"))?;

    let frame_raw = load_image(&input_path)?;
    let mut frame_display = frame_raw.try_clone()?;

    let layout = match args.layout_name.as_deref() {
        Some(name) => {
            let layout = find_layout_by_name(name)?;
            println!("Using specified layout: {}", layout.name);
            layout
        }
        None => {
            let layout = BottleLayout::detect_layout(&frame_raw)?;
            println!("Auto-detected layout: {}", layout.name);
            layout
        }
    };

    let bottles = detect_bottles_with_layout(&frame_raw, &mut frame_display, &layout)?;
    println!("Detected bottles: {}", bottles_to_sequence(&bottles));

    let output_path = match args.output {
        Some(path) => path,
        None => default_output_path()?,
    };

    save_image(&output_path, &frame_display)?;
    println!("Saved frame display to {}", output_path.display());

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
