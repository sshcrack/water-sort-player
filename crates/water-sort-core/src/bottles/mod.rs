use std::{collections::HashSet, fmt::Display, iter};

use colored::Colorize;
use opencv::core::{Mat, MatTraitConst, Vec3b};

pub mod detection;
pub mod empty_bottle_color_detection;
#[cfg(test)]
mod specific_tests;
pub mod test_utils;

use serde::{Deserialize, Serialize};

use crate::constants::BottleColor;

const DEFAULT_BOTTLE_CAPACITY: usize = 4;

#[derive(
    Debug, Clone, Serialize, Deserialize, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum HiddenRequirement {
    #[default]
    None,
    Locked(BottleColor),
    Unlocked(BottleColor),
}

#[derive(Debug, Clone, Deserialize, Serialize, Hash, PartialEq, Eq)]
pub struct Bottle {
    // Last element is the top color, first element is the bottom color
    // The boolean indicates whether the initial color was mystery, to properly handle filling in the solver
    fills: Vec<(BottleColor, bool)>,
    hidden_requirement: HiddenRequirement,
    /// This will always be set to Some for non test environments
    click_position: Option<crate::Pos>,
}

impl Bottle {
    fn normalize_fills_with_initial(
        fills: Vec<BottleColor>,
        initial: Vec<BottleColor>,
    ) -> Vec<(BottleColor, bool)> {
        fills
            .into_iter()
            .zip(
                initial
                    .into_iter()
                    .map(Some)
                    .chain(iter::repeat_with(|| None)),
            )
            .map(|(color, initial_color)| {
                (
                    color,
                    initial_color.is_some_and(|e| e == BottleColor::Mystery),
                )
            })
            .collect()
    }

    pub fn from_fills(fills: Vec<BottleColor>, click_position: Option<crate::Pos>) -> Self {
        Bottle {
            fills: fills.into_iter().map(|c| (c, false)).collect(),
            hidden_requirement: HiddenRequirement::None,
            click_position,
        }
    }

    pub fn from_fills_with_initial(
        fills: Vec<BottleColor>,
        initial: Vec<BottleColor>,
        click_position: Option<crate::Pos>,
    ) -> Self {
        Bottle {
            fills: Self::normalize_fills_with_initial(fills, initial),
            hidden_requirement: HiddenRequirement::None,
            click_position,
        }
    }

    pub fn from_hidden_requirement(
        requirement: BottleColor,
        click_position: Option<crate::Pos>,
    ) -> Self {
        Bottle {
            fills: vec![(BottleColor::Empty, false); DEFAULT_BOTTLE_CAPACITY],
            hidden_requirement: HiddenRequirement::Locked(requirement),
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

        let top_non_empty_index = self
            .fills
            .iter()
            .rposition(|(color, _)| !color.is_empty())?;

        let (top_color, top_was_mystery) = self.fills[top_non_empty_index];

        if top_was_mystery {
            return Some((1, top_color));
        }

        let mut amount = 1;

        for index in (0..top_non_empty_index).rev() {
            let (color, was_mystery) = self.fills[index];
            if color != top_color || color.is_empty() {
                break;
            }

            if was_mystery {
                break;
            }

            amount += 1;
        }

        Some((amount, top_color))
    }

    pub fn is_full(&self) -> bool {
        self.get_fill_count() >= self.get_capacity()
    }

    pub fn get_fill_count(&self) -> usize {
        self.fills
            .iter()
            .filter(|(color, _)| !color.is_empty())
            .count()
    }

    pub fn get_capacity(&self) -> usize {
        self.fills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.get_fill_count() == 0
    }

    pub fn is_solved(&self) -> bool {
        if self.is_hidden_and_locked() {
            return false;
        }

        if self.get_fill_count() != self.get_capacity() {
            return false;
        }

        let Some((first_color, _)) = self.fills.iter().find(|(color, _)| !color.is_empty()) else {
            return false;
        };

        self.fills.iter().all(|&(color, _)| {
            !color.is_empty() && color == *first_color && color != BottleColor::Mystery
        })
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

        if other_top_color == BottleColor::Mystery || other_top_color == BottleColor::Empty {
            return false;
        }

        let (_, self_top_color) = match self.get_top_fill() {
            Some((amount, color)) => (amount, color),
            None => {
                return true;
            }
        };

        if self_top_color == BottleColor::Mystery || self_top_color == BottleColor::Empty {
            return false;
        }

        if self.is_empty() {
            return true;
        }

        if self_top_color != other_top_color {
            return false;
        }

        self.get_fill_count() + other_top_amount <= self.get_capacity()
    }

    pub fn fill_from(&mut self, source: &mut Bottle) {
        if !self.can_fill_from(source) {
            panic!("Cannot fill from the given source bottle");
        }

        let (source_top_amount, source_top_color) = source.get_top_fill().unwrap();

        let destination_fill_count = self.get_fill_count();
        let source_fill_count = source.get_fill_count();
        let destination_available_space =
            self.get_capacity().saturating_sub(destination_fill_count);
        if destination_available_space < source_top_amount {
            panic!("Not enough space in the destination bottle to fill from the source");
        }

        let dest_idx_start = destination_fill_count;
        let dest_idx_end = destination_fill_count + source_top_amount;
        for index in dest_idx_start..dest_idx_end {
            self.fills[index] = (source_top_color, false);
        }

        let source_idx_start = source_fill_count - source_top_amount;
        let source_idx_end = source_fill_count;

        for index in source_idx_start..source_idx_end {
            source.fills[index] = (BottleColor::Empty, false);
        }
    }

    pub fn click_position(&self) -> &Option<crate::Pos> {
        &self.click_position
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        Self::from_fills(
            vec![
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::Empty,
            ],
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    #[test_log::test]
    fn mystery_pour() {
        use super::BottleColor;
        let mut empty_bottle = super::Bottle::from_fills(
            vec![
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::Empty,
            ],
            None,
        );
        let mut mystery_bottle = super::Bottle::from_fills_with_initial(
            vec![
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::orange(),
                BottleColor::orange(),
            ],
            vec![
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::Mystery,
                BottleColor::Mystery,
            ],
            None,
        );

        println!("Mystery top fill: {:?}", mystery_bottle.get_top_fill());
        empty_bottle.fill_from(&mut mystery_bottle);
        assert_eq!(
            empty_bottle.get_fills(),
            vec![
                BottleColor::orange(),
                BottleColor::Empty,
                BottleColor::Empty,
                BottleColor::Empty,
            ]
        )
    }

    #[test_log::test]
    fn top_mystery_returns_single_fill() {
        use super::BottleColor;

        let bottle = super::Bottle::from_fills_with_initial(
            vec![
                BottleColor::orange(),
                BottleColor::orange(),
                BottleColor::Empty,
                BottleColor::Empty,
            ],
            vec![
                BottleColor::orange(),
                BottleColor::Mystery,
                BottleColor::Empty,
                BottleColor::Empty,
            ],
            None,
        );

        assert_eq!(bottle.get_top_fill(), Some((1, BottleColor::orange())));
    }
}

impl Display for Bottle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
                write!(f, "{}{},{}", "!".red(), requirement, fill_str)
            }
            HiddenRequirement::Unlocked(requirement) => {
                write!(f, "{}{},{}", "!".green(), requirement, fill_str)
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
    seen_colors: &mut HashSet<BottleColor>,
) -> anyhow::Result<Vec<Bottle>> {
    detection::detect_bottles(frame_raw, frame_display, seen_colors)
}
