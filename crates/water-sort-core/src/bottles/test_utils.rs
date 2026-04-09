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
    ) -> anyhow::Result<Vec<Bottle>> {
        let mut frame_display = image.try_clone()?;
        detect_bottles_with_layout(image, &mut frame_display, layout)
    }

    pub fn parse_bottle_string(bottle_str: &str) -> Vec<BottleColor> {
        let mut fills: Vec<BottleColor> = bottle_str
            .chars()
            .filter_map(|c| match c {
                'Y' => Some(BottleColor::Yellow),
                'R' => Some(BottleColor::Red),
                'G' => Some(BottleColor::Green),
                'g' => Some(BottleColor::Lime),
                'L' => Some(BottleColor::LightBlue),
                'M' => Some(BottleColor::MediumBlue),
                'B' => Some(BottleColor::Blue),
                'P' => Some(BottleColor::Purple),
                'O' => Some(BottleColor::Orange),
                'W' => Some(BottleColor::Pink),
                '?' => Some(BottleColor::Mystery),
                'E' => None,
                _ => panic!("Invalid character in bottle string: {}", c),
            })
            .collect();

        // Strings are provided top->bottom; bottle fills are stored bottom->top.
        fills.reverse();
        fills
    }

    pub fn parse_bottles_sequence(sequence: &str) -> Vec<Bottle> {
        sequence
            .split_whitespace()
            .map(TestUtils::parse_bottle_string)
            .map(Bottle::from_fills)
            .collect()
    }
}
