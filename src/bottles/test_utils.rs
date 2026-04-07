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

    /// Validate that a detected bottle configuration makes sense
    pub fn validate_detection(bottles: &[Bottle]) -> DetectionResult {
        let mut result = DetectionResult {
            is_valid: true,
            bottle_count: bottles.len(),
            filled_bottles: 0,
            empty_bottles: 0,
            issues: Vec::new(),
        };

        for (idx, bottle) in bottles.iter().enumerate() {
            if bottle.is_empty() {
                result.empty_bottles += 1;
            } else {
                result.filled_bottles += 1;
            }

            // Check for reasonable fill counts (0-4)
            if bottle.get_fill_count() > 4 {
                result.is_valid = false;
                result.issues.push(format!(
                    "Bottle {} has {} fills (max 4 expected)",
                    idx,
                    bottle.get_fill_count()
                ));
            }
        }

        // Basic sanity check: should have some filled bottles
        if result.filled_bottles == 0 && result.bottle_count > 0 {
            result.is_valid = false;
            result
                .issues
                .push("No filled bottles detected - this seems unlikely".to_string());
        }

        result
    }
}

#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub is_valid: bool,
    pub bottle_count: usize,
    pub filled_bottles: usize,
    pub empty_bottles: usize,
    pub issues: Vec<String>,
}

impl DetectionResult {
    pub fn print_summary(&self) {
        println!("Detection Result:");
        println!("  Valid: {}", self.is_valid);
        println!("  Total bottles: {}", self.bottle_count);
        println!("  Filled bottles: {}", self.filled_bottles);
        println!("  Empty bottles: {}", self.empty_bottles);

        if !self.issues.is_empty() {
            println!("  Issues:");
            for issue in &self.issues {
                println!("    - {}", issue);
            }
        }
    }
}

/// Expected bottle configurations for test validation
#[derive(Debug, Clone)]
pub struct ExpectedBottles {
    pub total_bottles: usize,
    pub min_filled_bottles: usize,
    pub max_filled_bottles: usize,
}

impl ExpectedBottles {
    pub fn for_10_bottle_layout() -> Self {
        Self {
            total_bottles: 10,
            min_filled_bottles: 8, // Usually 8 filled + 2 empty
            max_filled_bottles: 10,
        }
    }

    pub fn for_12_bottle_layout() -> Self {
        Self {
            total_bottles: 11, // Actually 11 bottles visible
            min_filled_bottles: 9,
            max_filled_bottles: 11,
        }
    }

    pub fn validate(&self, result: &DetectionResult) -> bool {
        result.is_valid
            && result.bottle_count == self.total_bottles
            && result.filled_bottles >= self.min_filled_bottles
            && result.filled_bottles <= self.max_filled_bottles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detection_result_validation() {
        let expected = ExpectedBottles::for_10_bottle_layout();

        // Valid result
        let valid_result = DetectionResult {
            is_valid: true,
            bottle_count: 10,
            filled_bottles: 8,
            empty_bottles: 2,
            issues: Vec::new(),
        };
        assert!(expected.validate(&valid_result));

        // Invalid - wrong bottle count
        let invalid_result = DetectionResult {
            is_valid: true,
            bottle_count: 12,
            filled_bottles: 8,
            empty_bottles: 4,
            issues: Vec::new(),
        };
        assert!(!expected.validate(&invalid_result));
    }
}
