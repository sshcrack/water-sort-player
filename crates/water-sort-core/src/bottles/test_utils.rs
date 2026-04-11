use std::{env, fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

use crate::bottles::{Bottle, BottleLayout};
use crate::constants::BottleColor;
use crate::detect_bottles_with_layout;
use opencv::{core::Mat, imgcodecs, prelude::*};

/// Test utilities for bottle detection validation
pub struct TestUtils;

impl TestUtils {
    /// Load an image from the captures directory
    pub fn load_test_image(filename: &str) -> anyhow::Result<Mat> {
        let path = format!("../../captures/{}", filename);
        let img = imgcodecs::imread(&path, imgcodecs::IMREAD_COLOR)?;
        if img.empty() {
            return Err(anyhow::anyhow!("Failed to load image: {}", path));
        }
        Ok(img)
    }

    /// Run bottle detection on a test image without drawing
    pub fn detect_bottles_from_image(
        image: &Mat,
        layout: &BottleLayout,
        debug_filename_prefix: &str,
    ) -> anyhow::Result<Vec<Bottle>> {
        let mut frame_display = image.try_clone()?;
        let detection_result = detect_bottles_with_layout(image, &mut frame_display, layout);
        let saved_path = Self::save_test_debug_image(&frame_display, debug_filename_prefix)?;
        println!(
            "Saved bottle detection debug image to {}",
            saved_path.display()
        );

        detection_result
    }

    pub fn save_test_debug_image(
        image: &Mat,
        debug_filename_prefix: &str,
    ) -> anyhow::Result<PathBuf> {
        let parent_dir = env::current_exe()?
            .parent()
            .ok_or_else(|| anyhow::anyhow!("test executable has no parent directory"))?
            .to_path_buf();

        fs::create_dir_all(&parent_dir)?;

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        let filename = format!("{debug_filename_prefix}-{timestamp}.png");
        let path = parent_dir.join(filename);

        imgcodecs::imwrite(
            path.to_string_lossy().as_ref(),
            image,
            &opencv::core::Vector::new(),
        )?;

        Ok(path)
    }

    pub fn parse_bottle_string(bottle_str: &str) -> Vec<BottleColor> {
        bottle_str
            .chars()
            .filter_map(|c| match c {
                'Y' => Some((BottleColor::Yellow, false)),
                'R' => Some((BottleColor::Red, false)),
                'G' => Some((BottleColor::Green, false)),
                'g' => Some((BottleColor::Lime, false)),
                'L' => Some((BottleColor::LightBlue, false)),
                'M' => Some((BottleColor::MediumBlue, false)),
                'B' => Some((BottleColor::Blue, false)),
                'P' => Some((BottleColor::Purple, false)),
                'O' => Some((BottleColor::Orange, false)),
                'W' => Some((BottleColor::Pink, false)),
                '?' => Some((BottleColor::Mystery, true)),
                'E' => None,
                _ => panic!("Invalid character in bottle string: {}", c),
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|(color, _)| color)
            .collect()
    }

    pub fn parse_bottles_sequence(sequence: &str) -> Vec<Bottle> {
        sequence
            .split_whitespace()
            .map(|bottle_str| {
                let parsed = bottle_str
                    .chars()
                    .filter_map(|c| match c {
                        'Y' => Some((BottleColor::Yellow, false)),
                        'R' => Some((BottleColor::Red, false)),
                        'G' => Some((BottleColor::Green, false)),
                        'g' => Some((BottleColor::Lime, false)),
                        'L' => Some((BottleColor::LightBlue, false)),
                        'M' => Some((BottleColor::MediumBlue, false)),
                        'B' => Some((BottleColor::Blue, false)),
                        'P' => Some((BottleColor::Purple, false)),
                        'O' => Some((BottleColor::Orange, false)),
                        'W' => Some((BottleColor::Pink, false)),
                        '?' => Some((BottleColor::Mystery, true)),
                        'E' => None,
                        _ => panic!("Invalid character in bottle string: {}", c),
                    })
                    .collect::<Vec<_>>();

                let (fills, mystery_origin_flags): (Vec<_>, Vec<_>) =
                    parsed.into_iter().rev().unzip();

                // Strings are provided top->bottom; bottle fills are stored bottom->top.
                Bottle::from_fills_with_mystery_flags(fills, mystery_origin_flags)
            })
            .collect()
    }
}
