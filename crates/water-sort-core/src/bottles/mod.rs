use std::{fmt::Display, iter};

use colored::Colorize;
use opencv::core::{Mat, MatTraitConst, Vec3b};

#[cfg(test)]
mod specific_tests;
pub mod test_utils;

use serde::Serialize;

use crate::constants::BottleColor;

#[derive(Debug, Clone, Serialize, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum HiddenRequirement {
    #[default]
    None,
    Locked(BottleColor),
    Unlocked(BottleColor),
}

#[derive(Debug, Clone, Serialize, Hash, PartialEq, Eq, Default)]
pub struct Bottle {
    // Last element is the top color, first element is the bottom color
    // The boolean indicates whether the initial color was mystery, to properly handle filling in the solver
    fills: Vec<(BottleColor, bool)>,
    hidden_requirement: HiddenRequirement,
    /// This will always be set to Some for non test environments
    click_position: Option<crate::Pos>,
}

// Remove hardcoded constants - using layouts now
const FULL_BOTTLE_COUNT: usize = 4;

impl Bottle {
    pub fn from_fills(fills: Vec<BottleColor>, click_position: Option<crate::Pos>) -> Self {
        Bottle {
            fills: fills
                .into_iter()
                .map(|color| (color, color == BottleColor::Mystery))
                .collect(),
            hidden_requirement: HiddenRequirement::None,
            click_position,
        }
    }

    pub fn from_fills_with_initial(
        fills: Vec<BottleColor>,
        initial: Vec<BottleColor>,
        click_position: Option<crate::Pos>,
    ) -> Self {
        let initial_with_fallback = initial
            .into_iter()
            .map(Some)
            .chain(iter::repeat_with(|| None));

        Bottle {
            fills: fills
                .into_iter()
                .zip(initial_with_fallback)
                .map(|(color, initial_color)| {
                    (
                        color,
                        initial_color.is_some_and(|e| e == BottleColor::Mystery),
                    )
                })
                .collect(),
            hidden_requirement: HiddenRequirement::None,
            click_position,
        }
    }

    pub fn from_hidden_requirement(
        requirement: BottleColor,
        click_position: Option<crate::Pos>,
    ) -> Self {
        Bottle {
            fills: Vec::new(),
            hidden_requirement: HiddenRequirement::Locked(requirement),
            click_position,
        }
    }

    pub fn from_unlocked_hidden_requirement(
        requirement: BottleColor,
        click_position: Option<crate::Pos>,
    ) -> Self {
        Bottle {
            fills: Vec::new(),
            hidden_requirement: HiddenRequirement::Unlocked(requirement),
            click_position,
        }
    }

    pub fn hidden_requirement_state(&self) -> HiddenRequirement {
        self.hidden_requirement
    }

    pub fn hidden_requirement(&self) -> Option<BottleColor> {
        match self.hidden_requirement {
            HiddenRequirement::Locked(requirement) | HiddenRequirement::Unlocked(requirement) => {
                Some(requirement)
            }
            HiddenRequirement::None => None,
        }
    }

    pub fn get_locked_hidden_requirement(&self) -> Option<BottleColor> {
        match self.hidden_requirement {
            HiddenRequirement::Locked(requirement) => Some(requirement),
            HiddenRequirement::None | HiddenRequirement::Unlocked(_) => None,
        }
    }

    pub fn is_hidden_and_locked(&self) -> bool {
        matches!(self.hidden_requirement, HiddenRequirement::Locked(_))
    }

    pub fn set_hidden_requirement(&mut self, requirement: HiddenRequirement) {
        self.hidden_requirement = requirement;
    }

    pub fn unlock_hidden_requirement(&mut self) {
        if let HiddenRequirement::Locked(requirement) = self.hidden_requirement {
            self.hidden_requirement = HiddenRequirement::Unlocked(requirement);
        }
    }
    pub fn lock_hidden_requirement(&mut self) {
        if let HiddenRequirement::Unlocked(requirement) = self.hidden_requirement {
            self.hidden_requirement = HiddenRequirement::Locked(requirement);
        }
    }

    pub fn get_fills_mut(&mut self) -> &mut Vec<(BottleColor, bool)> {
        &mut self.fills
    }

    pub fn set_fills_from_bottle(&mut self, other: &Bottle) {
        self.fills = other.fills.clone();
    }

    pub fn get_fills(&self) -> Vec<BottleColor> {
        self.fills.iter().map(|(color, _)| *color).collect()
    }

    pub fn get_top_fill(&self) -> Option<(usize, BottleColor)> {
        if self.is_hidden_and_locked() {
            return None;
        }

        let mut last_fill = None;
        let mut amount = 0;

        for (i, (color, was_mystery)) in self.fills.iter().rev().enumerate() {
            if i == 0 {
                last_fill = Some(color);
                amount = 1;
                if *was_mystery {
                    break;
                }
            } else if Some(color) == last_fill {
                if *was_mystery {
                    break;
                }
                amount += 1;
            } else {
                break;
            }
        }

        last_fill.map(|color| (amount, *color))
    }

    pub fn is_full(&self) -> bool {
        self.fills.len() >= FULL_BOTTLE_COUNT
    }

    pub fn get_fill_count(&self) -> usize {
        self.fills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fills.is_empty()
    }

    pub fn is_solved(&self) -> bool {
        if self.is_hidden_and_locked() {
            return false;
        }

        if self.get_fill_count() != FULL_BOTTLE_COUNT {
            return false;
        }

        let (first_color, _) = self.fills[0];
        self.fills
            .iter()
            .all(|&(color, _)| color == first_color && color != BottleColor::Mystery)
    }

    pub fn solved_color(&self) -> Option<BottleColor> {
        if self.is_solved() {
            self.fills.first().map(|(color, _)| *color)
        } else {
            None
        }
    }

    pub fn can_fill_from(&self, other: &Bottle) -> bool {
        if self.is_hidden_and_locked() || other.is_hidden_and_locked() {
            return false;
        }

        if self.is_full() || other.is_empty() {
            return false;
        }

        let (other_top_amount, other_top_color) = match other.get_top_fill() {
            Some((amount, color)) => (amount, color),
            None => return false,
        };

        if self.is_empty() {
            return true;
        }

        let (_, self_top_color) = match self.get_top_fill() {
            Some((amount, color)) => (amount, color),
            None => {
                unreachable!("This should never happen since we already checked if self is empty")
            }
        };

        if self_top_color != other_top_color {
            return false;
        }

        if self_top_color == BottleColor::Mystery || other_top_color == BottleColor::Mystery {
            return false;
        }

        self.get_fill_count() + other_top_amount <= FULL_BOTTLE_COUNT
    }

    pub fn fill_from(&mut self, source: &mut Bottle) {
        if !self.can_fill_from(source) {
            panic!("Cannot fill from the given source bottle");
        }

        let (source_top_amount, source_top_color) = source.get_top_fill().unwrap();

        let available_space = FULL_BOTTLE_COUNT - self.get_fill_count();
        if available_space < source_top_amount {
            panic!("Not enough space in the destination bottle to fill from the source");
        }

        for _ in 0..source_top_amount {
            self.fills.push((source_top_color, false));
            source.fills.pop();
        }
    }
    
    pub fn click_position(&self) -> &Option<crate::Pos> {
        &self.click_position
    }
}

impl Display for Bottle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return match self.hidden_requirement {
                HiddenRequirement::None => write!(f, "EEEE"),
                HiddenRequirement::Locked(bottle_color) => {
                    write!(f, "!{}", bottle_color.to_string().red())
                }
                HiddenRequirement::Unlocked(bottle_color) => {
                    write!(f, "!{},EEEE", bottle_color.to_string().green())
                }
            };
        }

        let fill_str: String = self
            .fills
            .iter()
            .rev()
            .map(|(color, was_mystery)| {
                let c = color.to_string();

                if *was_mystery {
                    c.underline().to_string()
                } else {
                    c
                }
            })
            .collect();

        match self.hidden_requirement {
            HiddenRequirement::Locked(requirement) => {
                write!(f, "!{},{}", requirement.to_string().red(), fill_str)
            }
            HiddenRequirement::Unlocked(requirement) => {
                write!(f, "!{},{}", requirement.to_string().green(), fill_str)
            }
            HiddenRequirement::None => write!(f, "{}", fill_str),
        }
    }
}

pub fn has_failed_level(image: &Mat) -> anyhow::Result<bool> {
    let failed_color = image.at_2d::<Vec3b>(
        crate::constants::FAILED_LEVEL_TEXT.1,
        crate::constants::FAILED_LEVEL_TEXT.0,
    )?;
    Ok(
        crate::constants::color_distance_sq(failed_color, &crate::constants::FAILED_LEVEL_COLOR)
            <= crate::constants::COLOR_DISTANCE_THRESHOLD_SQ,
    )
}

pub fn detect_bottles(
    frame_raw: &Mat,
    frame_display: &mut Mat,
    seen_colors: Option<&[Vec<BottleColor>]>,
) -> anyhow::Result<Vec<Bottle>> {
    let mut bottles: Vec<Bottle> = Vec::new();

    // If seen colors is set, try to match the detected bottles colors to the seen colors and if they are not in a reasonable HSV range, use the avg color value of them instead

    todo!()
}
