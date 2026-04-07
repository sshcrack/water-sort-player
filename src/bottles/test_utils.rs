use crate::bottles::{Bottle, BottleLayout};
use crate::constants::BottleColor;
use opencv::{core::Mat, imgcodecs, prelude::*};

/// Test utilities for bottle detection validation
pub struct TestUtils;

impl TestUtils {
    /// Load an image from the captures directory
    pub fn load_test_image(filename: &str) -> anyhow::Result<Mat> {
        let path = format!("captures/{}", filename);
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
        let mut bottles = Vec::new();

        // Initialize bottles for this layout
        for _ in 0..layout.bottle_count() {
            bottles.push(Bottle::default());
        }

        // Detect colors for each bottle
        for bottle_idx in 0..layout.bottle_count() {
            // Try to find 4 layers for each bottle (standard bottle capacity)
            for layer_idx in 0..4 {
                if let Some(sample_pos) = layout.get_sample_position(bottle_idx, layer_idx) {
                    let x = sample_pos.0;
                    let y = sample_pos.1;

                    // Check if coordinates are within image bounds
                    if y >= 0 && y < image.rows() && x >= 0 && x < image.cols() {
                        let pixel = image.at_2d::<opencv::core::Vec3b>(y, x)?;

                        if let Some(color) = BottleColor::from_pixel_value(*pixel) {
                            bottles[bottle_idx].fills.push(color);
                        }
                        // Skip empty pixels and unknown colors
                    }
                }
            }
        }

        // Reverse fills so bottom colors are at index 0
        for bottle in &mut bottles {
            bottle.fills.reverse();
        }

        Ok(bottles)
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

    pub fn parse_bottles_sequence(sequence: &str) -> Vec<Vec<BottleColor>> {
        sequence
            .split_whitespace()
            .map(TestUtils::parse_bottle_string)
            .collect()
    }
}
