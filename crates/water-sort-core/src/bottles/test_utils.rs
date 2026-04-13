use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::bottles::{Bottle, BottleLayout, HiddenRequirement};
use crate::constants::BottleColor;
use crate::detect_bottles_with_layout;
use opencv::{core::Mat, imgcodecs, prelude::*};

/// Test utilities for bottle detection validation
pub struct TestUtils;

impl TestUtils {
    fn parse_color_char(c: char) -> BottleColor {
        match c {
            'Y' => BottleColor::Yellow,
            'R' => BottleColor::Red,
            'G' => BottleColor::Green,
            'g' => BottleColor::Lime,
            'L' => BottleColor::LightBlue,
            'M' => BottleColor::MediumBlue,
            'B' => BottleColor::Blue,
            'P' => BottleColor::Purple,
            'O' => BottleColor::Orange,
            'W' => BottleColor::Pink,
            '?' => BottleColor::Mystery,
            _ => panic!("Invalid color character in bottle string: {}", c),
        }
    }

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

    fn parse_bottle_string(bottle_str: &str) -> Vec<BottleColor> {
        let mut fills: Vec<BottleColor> = bottle_str
            .chars()
            .filter_map(|c| match c {
                'Y' | 'R' | 'G' | 'g' | 'L' | 'M' | 'B' | 'P' | 'O' | 'W' | '?' => {
                    Some(Self::parse_color_char(c))
                }
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
            .map(|token| {
                if token.starts_with('!') {
                    let (requirement_token, fills_token) = token
                        .split_once(',')
                        .map_or((token, None), |(requirement, fills)| {
                            (requirement, Some(fills))
                        });

                    let requirement_char = requirement_token
                        .chars()
                        .nth(1)
                        .expect("Hidden bottle requirement token is missing a color");
                    let requirement = Self::parse_color_char(requirement_char);

                    let mut bottle = if let Some(fills) = fills_token {
                        Bottle::from_fills(TestUtils::parse_bottle_string(fills))
                    } else {
                        Bottle::from_hidden_requirement(requirement)
                    };

                    bottle.set_hidden_requirement(if fills_token.is_some() {
                        HiddenRequirement::Unlocked(requirement)
                    } else {
                        HiddenRequirement::Locked(requirement)
                    });
                    bottle
                } else {
                    Bottle::from_fills(TestUtils::parse_bottle_string(token))
                }
            })
            .collect()
    }

    pub fn are_bottles_equal(a: &[Bottle], expected: &[Bottle]) -> bool {
        if a.len() != expected.len() {
            println!(
                "Bottle sequences have different lengths: {} vs {}",
                a.len(),
                expected.len()
            );
            return false;
        }

        for (i, (bottle_a, bottle_b)) in a.iter().zip(expected.iter()).enumerate() {
            if bottle_a.get_fills() != bottle_b.get_fills() {
                println!(
                    "Bottles differ at index {}: {:?}, expected {:?}",
                    i, bottle_a, bottle_b
                );
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::TestUtils;
    use crate::bottles::HiddenRequirement;
    use crate::constants::BottleColor;

    #[test_log::test]
    fn parse_bottles_sequence_supports_hidden_requirement_tokens() {
        let bottles = TestUtils::parse_bottles_sequence("OR OB EEEE !O !B");

        assert_eq!(bottles.len(), 5);
        assert_eq!(
            bottles[0].get_fills(),
            vec![BottleColor::Red, BottleColor::Orange]
        );
        assert_eq!(
            bottles[1].get_fills(),
            vec![BottleColor::Blue, BottleColor::Orange]
        );
        assert!(bottles[2].is_empty());
        assert_eq!(bottles[3].hidden_requirement(), Some(BottleColor::Orange));
        assert_eq!(bottles[4].hidden_requirement(), Some(BottleColor::Blue));
        assert_eq!(
            bottles[3].hidden_requirement_state(),
            HiddenRequirement::Locked(BottleColor::Orange)
        );
        assert_eq!(
            bottles[4].hidden_requirement_state(),
            HiddenRequirement::Locked(BottleColor::Blue)
        );
    }

    #[test_log::test]
    fn parse_bottles_sequence_supports_hidden_requirement_with_fills_tokens() {
        let bottles =
            TestUtils::parse_bottles_sequence("GGWW !P,YGRB PWBB !O,RYPW OYYB RPOP OORG EEEE EEEE");

        assert_eq!(bottles.len(), 9);
        assert_eq!(bottles[1].hidden_requirement(), Some(BottleColor::Purple));
        assert_eq!(
            bottles[1].hidden_requirement_state(),
            HiddenRequirement::Unlocked(BottleColor::Purple)
        );
        assert_eq!(
            bottles[1].get_fills(),
            vec![
                BottleColor::Blue,
                BottleColor::Red,
                BottleColor::Green,
                BottleColor::Yellow,
            ]
        );
        assert_eq!(bottles[3].hidden_requirement(), Some(BottleColor::Orange));
        assert_eq!(
            bottles[3].hidden_requirement_state(),
            HiddenRequirement::Unlocked(BottleColor::Orange)
        );
        assert_eq!(
            bottles[3].get_fills(),
            vec![
                BottleColor::Pink,
                BottleColor::Purple,
                BottleColor::Yellow,
                BottleColor::Red,
            ]
        );
    }
}
