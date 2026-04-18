use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::constants::BottleColor;
use crate::{
    bottles::{Bottle, HiddenRequirement},
    detect_bottles,
};
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
        debug_filename_prefix: &str,
    ) -> anyhow::Result<Vec<Bottle>> {
        let mut frame_display = image.try_clone()?;
        let detection_result = detect_bottles(image, &mut frame_display, None);
        let saved_path = Self::save_test_debug_image(&frame_display, debug_filename_prefix)?;
        println!(
            "Saved bottle detection debug image to {}",
            saved_path.display()
        );

        detection_result
    }

    pub fn load_bottles_from_state<P: AsRef<Path>>(state_path: P) -> (Vec<Bottle>, Vec<Bottle>) {
        let state_path = state_path.as_ref();
        let state_str = fs::read_to_string(state_path).unwrap_or_else(|_| {
            panic!("Failed to read bottle state from {}", state_path.display())
        });

        let json = serde_json::from_str::<serde_json::Value>(&state_str).unwrap_or_else(|_| {
            panic!(
                "Failed to parse bottle state JSON from {}",
                state_path.display()
            )
        });

        let app_state = json
            .get("app_state")
            .expect("Missing 'app_state' field in JSON");
        let state = app_state
            .as_object()
            .expect("'app_state' should be a JSON object")
            .values()
            .next()
            .expect("No state found in 'app_state'");

        let initial_state = state
            .get("initial_state")
            .expect("Missing 'initial_state' field in state");

        let max_revealed_colors = state
            .get("max_revealed_bottle_state")
            .expect("Missing 'max_revealed_bottle_state' field in state");
        let initial_bottles = serde_json::from_value::<Vec<Bottle>>(initial_state.clone())
            .expect("Failed to deserialize initial bottle state from JSON");
        let max_revealed_bottles =
            serde_json::from_value::<Vec<Bottle>>(max_revealed_colors.clone())
                .expect("Failed to deserialize max revealed bottle state from JSON");

        (initial_bottles, max_revealed_bottles)
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

    fn bottle_color_from_char(c: char) -> BottleColor {
        match c {
            'E' => BottleColor::Empty,
            '?' => BottleColor::Mystery,
            'R' => BottleColor::red(),
            'O' => BottleColor::orange(),
            'Y' => BottleColor::yellow(),
            'G' => BottleColor::green(),
            'g' => BottleColor::light_green(),
            'B' => BottleColor::blue(),
            'D' => BottleColor::medium_blue(),
            'M' => BottleColor::medium_blue(),
            'P' => BottleColor::purple(),
            'W' => BottleColor::pink(),
            'L' => BottleColor::light_blue(),
            _ => panic!("Invalid bottle color character: {}", c),
        }
    }

    fn parse_bottle_string_old_format(bottle_str: &str) -> Vec<BottleColor> {
        let mut fills: Vec<BottleColor> = bottle_str
            .chars()
            .map(Self::bottle_color_from_char)
            .collect();

        // Strings are provided top->bottom; bottle fills are stored bottom->top.
        fills.reverse();
        fills
    }

    pub fn parse_bottles_sequence(bottle_str: &str) -> Vec<Bottle> {
        let is_new_format = bottle_str.contains('#');
        if is_new_format {
            Self::parse_bottles_sequence_new_format(bottle_str)
        } else {
            Self::parse_bottles_sequence_old_format(bottle_str)
        }
    }

    fn parse_bottle_string_new_format(bottle_str: &str) -> Vec<BottleColor> {
        if bottle_str.chars().all(|c| c == 'E') {
            return vec![BottleColor::Empty; 4];
        }

        let mut fills: Vec<BottleColor> = bottle_str
            .split('#')
            .filter_map(|part| {
                if part.is_empty() || part == "E" {
                    None
                } else {
                    Some(BottleColor::from_hex(&format!("#{}", part)))
                }
            })
            .collect();

        // Strings are provided top->bottom; bottle fills are stored bottom->top.
        fills.reverse();
        fills
    }

    pub fn parse_bottles_sequence_new_format(sequence: &str) -> Vec<Bottle> {
        sequence
            .split_whitespace()
            .map(|token| {
                if token.starts_with('!') {
                    let (requirement_token, fills_token) = token
                        .split_once(',')
                        .map_or((token, None), |(requirement, fills)| {
                            (requirement, Some(fills))
                        });

                    // requirement_token is like "!#f37c1c"
                    let requirement_hex = requirement_token.trim_start_matches('!');
                    let requirement = BottleColor::from_hex(requirement_hex);

                    let mut bottle = if let Some(fills) = fills_token {
                        Bottle::from_fills(TestUtils::parse_bottle_string_new_format(fills), None)
                    } else {
                        Bottle::from_hidden_requirement(requirement, None)
                    };

                    bottle.set_hidden_requirement(if fills_token.is_some() {
                        HiddenRequirement::Unlocked(requirement)
                    } else {
                        HiddenRequirement::Locked(requirement)
                    });
                    bottle
                } else {
                    Bottle::from_fills(TestUtils::parse_bottle_string_new_format(token), None)
                }
            })
            .collect()
    }

    pub fn parse_bottles_sequence_old_format(sequence: &str) -> Vec<Bottle> {
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
                    let requirement = Self::bottle_color_from_char(requirement_char);

                    let mut bottle = if let Some(fills) = fills_token {
                        Bottle::from_fills(TestUtils::parse_bottle_string_old_format(fills), None)
                    } else {
                        Bottle::from_hidden_requirement(requirement, None)
                    };

                    bottle.set_hidden_requirement(if fills_token.is_some() {
                        HiddenRequirement::Unlocked(requirement)
                    } else {
                        HiddenRequirement::Locked(requirement)
                    });
                    bottle
                } else {
                    Bottle::from_fills(TestUtils::parse_bottle_string_old_format(token), None)
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
        let bottles = TestUtils::parse_bottles_sequence(
            "#df1a24#f37c1c #194af9#f37c1c EEEE !#f37c1c !#194af9",
        );

        assert_eq!(bottles.len(), 5);
        assert_eq!(
            bottles[0].get_fills(),
            vec![BottleColor::red(), BottleColor::orange()]
        );
        assert_eq!(
            bottles[1].get_fills(),
            vec![BottleColor::blue(), BottleColor::orange()]
        );
        assert!(bottles[2].is_empty());
        assert_eq!(bottles[3].hidden_requirement(), Some(BottleColor::orange()));
        assert_eq!(bottles[4].hidden_requirement(), Some(BottleColor::blue()));
        assert_eq!(
            bottles[3].hidden_requirement_state(),
            HiddenRequirement::Locked(BottleColor::orange())
        );
        assert_eq!(
            bottles[4].hidden_requirement_state(),
            HiddenRequirement::Locked(BottleColor::blue())
        );
    }

    #[test_log::test]
    fn parse_bottles_sequence_supports_hidden_requirement_with_fills_tokens() {
        let bottles = TestUtils::parse_bottles_sequence(
            "#46de1e#46de1e#d212cc#d212cc !#8c00d9,#fbdf20#46de1e#df1a24#194af9 #8c00d9#d212cc#194af9#194af9 !#f37c1c,#df1a24#fbdf20#8c00d9#d212cc #f37c1c#fbdf20#fbdf20#194af9 #df1a24#8c00d9#f37c1c#f37c1c #f37c1c#f37c1c#df1a24#46de1e EEEE EEEE",
        );

        assert_eq!(bottles.len(), 9);
        assert_eq!(bottles[1].hidden_requirement(), Some(BottleColor::purple()));
        assert_eq!(
            bottles[1].hidden_requirement_state(),
            HiddenRequirement::Unlocked(BottleColor::purple())
        );
        assert_eq!(
            bottles[1].get_fills(),
            vec![
                BottleColor::blue(),
                BottleColor::red(),
                BottleColor::green(),
                BottleColor::yellow(),
            ]
        );
        assert_eq!(bottles[3].hidden_requirement(), Some(BottleColor::orange()));
        assert_eq!(
            bottles[3].hidden_requirement_state(),
            HiddenRequirement::Unlocked(BottleColor::orange())
        );
        assert_eq!(
            bottles[3].get_fills(),
            vec![
                BottleColor::pink(),
                BottleColor::purple(),
                BottleColor::yellow(),
                BottleColor::red(),
            ]
        );
    }
}
