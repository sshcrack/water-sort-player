use std::fmt::Display;

use opencv::{
    core::{Mat, MatTraitConst, Rect, Scalar, Vec3b},
    imgcodecs, imgproc,
};

mod layout;
#[cfg(test)]
mod specific_tests;
pub mod test_utils;

pub use layout::BottleLayout;

use crate::constants::{BottleColor, COLOR_DISTANCE_THRESHOLD_SQ, COLOR_VALUES, color_distance_sq};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerSample {
    Empty,
    Color(BottleColor),
    Unknown,
}

pub(crate) fn classify_bottle_layer(pixel: Vec3b, has_failed_level: bool) -> LayerSample {
    if BottleColor::is_empty_pixel(&pixel, has_failed_level) {
        LayerSample::Empty
    } else if let Some(color) = BottleColor::from_pixel_value(pixel, has_failed_level) {
        LayerSample::Color(color)
    } else {
        LayerSample::Unknown
    }
}

fn best_matching_surrounding_pixel(
    frame_raw: &Mat,
    center_x: i32,
    center_y: i32,
    radius: i32,
) -> anyhow::Result<Vec3b> {
    let min_x = (center_x - radius).max(0);
    let max_x = (center_x + radius).min(frame_raw.cols() - 1);
    let min_y = (center_y - radius).max(0);
    let max_y = (center_y + radius).min(frame_raw.rows() - 1);

    let mut best_pixel = *frame_raw.at_2d::<Vec3b>(center_y, center_x)?;
    let mut best_dist = u32::MAX;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let pixel = frame_raw.at_2d::<Vec3b>(y, x)?;

            let pixel_best_dist = COLOR_VALUES
                .iter()
                .map(|(_, target_pixel)| color_distance_sq(pixel, target_pixel))
                .min()
                .unwrap_or(u32::MAX);

            if pixel_best_dist < best_dist {
                best_dist = pixel_best_dist;
                best_pixel = *pixel;
            }
        }
    }

    Ok(best_pixel)
}

fn detect_hidden_requirement_color(
    frame_raw: &Mat,
    layout: &BottleLayout,
    bottle_idx: usize,
) -> anyhow::Result<Option<BottleColor>> {
    let Some(top_pos) = layout.get_sample_position(bottle_idx, 0) else {
        return Ok(None);
    };
    let Some(bottom_pos) = layout.get_sample_position(bottle_idx, 3) else {
        return Ok(None);
    };

    let center_x = top_pos.0;
    let search_radius_x = 24;
    let search_y_start = top_pos.1.saturating_add(18);
    let search_y_end = bottom_pos.1.saturating_sub(10);

    let mut best_color = None;
    let mut best_dist = u32::MAX;

    let min_x = (center_x - search_radius_x).max(0);
    let max_x = (center_x + search_radius_x).min(frame_raw.cols() - 1);
    let min_y = search_y_start.max(0).min(frame_raw.rows() - 1);
    let max_y = search_y_end.max(0).min(frame_raw.rows() - 1);

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let pixel = frame_raw.at_2d::<Vec3b>(y, x)?;
            for (color, target_pixel) in COLOR_VALUES.iter() {
                let dist = color_distance_sq(pixel, target_pixel);
                if dist < best_dist {
                    best_dist = dist;
                    best_color = Some(*color);
                }
            }
        }
    }

    Ok(match best_color {
        Some(color) if best_dist <= COLOR_DISTANCE_THRESHOLD_SQ => Some(color),
        _ => None,
    })
}

fn is_hidden_curtain_bottle(
    frame_raw: &Mat,
    layout: &BottleLayout,
    bottle_idx: usize,
) -> anyhow::Result<bool> {
    let Some(top_pos) = layout.get_sample_position(bottle_idx, 0) else {
        return Ok(false);
    };
    let Some(bottom_pos) = layout.get_sample_position(bottle_idx, 3) else {
        return Ok(false);
    };

    let curtain_reference = crate::constants::vec3_from_hex("#268072");
    let min_x = (top_pos.0 - 22).max(0);
    let max_x = (top_pos.0 + 22).min(frame_raw.cols() - 1);
    let min_y = (top_pos.1 - 8).max(0);
    let max_y = (bottom_pos.1 + 32).min(frame_raw.rows() - 1);

    let mut total_samples = 0usize;
    let mut curtain_like_samples = 0usize;

    for y in (min_y..=max_y).step_by(4) {
        for x in (min_x..=max_x).step_by(4) {
            let pixel = frame_raw.at_2d::<Vec3b>(y, x)?;
            total_samples += 1;

            let dist = color_distance_sq(pixel, &curtain_reference);
            let r = pixel[2] as i32;
            let g = pixel[1] as i32;
            let b = pixel[0] as i32;
            let teal_like = g > r + 25 && b > r + 15 && g > 75 && b > 65;

            if dist <= 80 * 80 || teal_like {
                curtain_like_samples += 1;
            }
        }
    }

    if total_samples == 0 {
        return Ok(false);
    }

    let curtain_ratio = curtain_like_samples as f32 / total_samples as f32;
    Ok(curtain_ratio >= 0.30)
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct Bottle {
    // Last element is the top color, first element is the bottom color
    // The boolean indicates whether the initial color was mystery, to properly handle filling in the solver
    fills: Vec<(BottleColor, bool)>,
    hidden_requirement: Option<BottleColor>,
}

// Remove hardcoded constants - using layouts now
const FULL_BOTTLE_COUNT: usize = 4;

impl Bottle {
    pub fn from_fills(fills: Vec<BottleColor>) -> Self {
        Bottle {
            fills: fills
                .into_iter()
                .map(|color| (color, color == BottleColor::Mystery))
                .collect(),
            hidden_requirement: None,
        }
    }

    pub fn from_fills_with_initial(fills: Vec<BottleColor>, initial: Vec<BottleColor>) -> Self {
        Bottle {
            fills: fills
                .into_iter()
                .zip(initial)
                .map(|(color, initial_color)| (color, initial_color == BottleColor::Mystery))
                .collect(),
            hidden_requirement: None,
        }
    }

    pub fn from_hidden_requirement(requirement: BottleColor) -> Self {
        Bottle {
            fills: Vec::new(),
            hidden_requirement: Some(requirement),
        }
    }

    pub fn hidden_requirement(&self) -> Option<BottleColor> {
        self.hidden_requirement
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden_requirement.is_some()
    }

    pub fn clear_hidden_requirement(&mut self) {
        self.hidden_requirement = None;
    }

    pub fn set_hidden_requirement(&mut self, requirement: Option<BottleColor>) {
        self.hidden_requirement = requirement;
    }

    pub fn get_fills_mut(&mut self) -> &mut Vec<(BottleColor, bool)> {
        &mut self.fills
    }

    pub fn get_fills(&self) -> Vec<BottleColor> {
        self.fills.iter().map(|(color, _)| *color).collect()
    }

    pub fn get_top_fill(&self) -> Option<(usize, BottleColor)> {
        if self.is_hidden() {
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
        if self.is_hidden() {
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
        if self.is_hidden() || other.is_hidden() {
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
            println!("Tried to fill mystery color into bottles");
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
}

impl Display for Bottle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "EEEE");
        }

        if let Some(requirement) = self.hidden_requirement {
            return write!(f, "!{}", requirement.to_char());
        }

        let fill_str: String = self
            .fills
            .iter()
            .rev()
            .map(|(color, was_mystery)| {
                let c = if *was_mystery { "?" } else { "" }.to_string();

                c + color.to_char().to_string().as_str()
            })
            .collect();

        if let Some(requirement) = self.hidden_requirement {
            write!(f, "!{}{}", requirement.to_char(), fill_str)
        } else {
            write!(f, "{}", fill_str)
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

pub fn detect_bottles_with_layout(
    frame_raw: &Mat,
    frame_display: &mut Mat,
    layout: &BottleLayout,
) -> anyhow::Result<Vec<Bottle>> {
    let mut bottles = Vec::new();

    // Initialize bottles for this layout
    for _ in 0..layout.bottle_count() {
        bottles.push(Bottle::default());
    }

    let has_failed_level = has_failed_level(frame_raw)?;

    let mut any_unknown = false;
    // Detect colors for each bottle
    for (bottle_idx, bottle) in bottles.iter_mut().enumerate().take(layout.bottle_count()) {
        let mut seen_content = false;
        let mut bottle_is_valid = true;
        let mut saw_unknown = false;
        let mut unresolved_unknown = false;

        // Try to find 4 layers for each bottle (standard bottle capacity)
        for layer_idx in 0..4 {
            if let Some(sample_pos) = layout.get_sample_position(bottle_idx, layer_idx) {
                let x = sample_pos.0;
                let y = sample_pos.1;

                // Draw detection rectangle for visualization
                imgproc::rectangle(
                    frame_display,
                    Rect::new(x - 20, y - 20, 40, 40),
                    Scalar::new(0.0, 255.0, 0.0, 255.0),
                    2,
                    imgproc::LINE_8,
                    0,
                )
                .unwrap();

                let best_pixel = best_matching_surrounding_pixel(frame_raw, x, y, 4)?;
                let sample = classify_bottle_layer(best_pixel, has_failed_level);

                match sample {
                    LayerSample::Empty => {
                        // Empty pixel - draw white marker
                        imgproc::rectangle(
                            frame_display,
                            Rect::new(x - 5, y - 5, 10, 10),
                            Scalar::new(255.0, 255.0, 255.0, 255.0),
                            2,
                            imgproc::LINE_8,
                            0,
                        )
                        .unwrap();

                        if seen_content {
                            bottle_is_valid = false;
                            break;
                        }
                    }
                    LayerSample::Color(color) => {
                        // Detected color - draw colored marker
                        imgproc::rectangle(
                            frame_display,
                            Rect::new(x - 5, y - 5, 10, 10),
                            color.to_pixel_value().into(),
                            2,
                            imgproc::LINE_8,
                            0,
                        )
                        .unwrap();
                        seen_content = true;
                        bottle.fills.push((color, false));
                    }
                    LayerSample::Unknown => {
                        saw_unknown = true;
                        unresolved_unknown = true;
                        let best_pixel_hex = format!(
                            "#{:02x}{:02x}{:02x}",
                            best_pixel[2], best_pixel[1], best_pixel[0]
                        );
                        println!(
                            "WARN: Pixel at ({}, {}) did not match any known color: {:?}. Treating bottle as invalid.",
                            x, y, best_pixel_hex
                        );
                        // Unknown color - draw black marker
                        imgproc::rectangle(
                            frame_display,
                            Rect::new(x - 5, y - 5, 10, 10),
                            Scalar::new(0.0, 0.0, 0.0, 255.0),
                            5,
                            imgproc::LINE_8,
                            0,
                        )
                        .unwrap();
                        break;
                    }
                }
            }
        }

        if saw_unknown && is_hidden_curtain_bottle(frame_raw, layout, bottle_idx)? {
            bottle.fills.clear();
            if let Some(requirement) =
                detect_hidden_requirement_color(frame_raw, layout, bottle_idx)?
            {
                bottle.set_hidden_requirement(Some(requirement));
                unresolved_unknown = false;
                bottle_is_valid = true;
            } else {
                bottle_is_valid = false;
                unresolved_unknown = true;
            }
        } else if bottle.fills.is_empty() && saw_unknown && !seen_content {
            bottle_is_valid = false;
            unresolved_unknown = true;
        }

        if !bottle_is_valid {
            bottle.fills.clear();
            any_unknown |= unresolved_unknown;
        }
    }

    if any_unknown {
        // Write out mat to file for debugging with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let raw_filename = format!("target/unknown_color_detection_{}_raw.png", timestamp);
        let display_filename = format!("target/unknown_color_detection_{}.png", timestamp);
        let _ = imgcodecs::imwrite(
            raw_filename.as_str(),
            frame_raw,
            &opencv::core::Vector::new(),
        );
        let _ = imgcodecs::imwrite(
            display_filename.as_str(),
            frame_display,
            &opencv::core::Vector::new(),
        );

        println!(
            "Detection files have been saved to {} and {}",
            raw_filename, display_filename
        );
        return Err(anyhow::anyhow!(
            "One or more pixels could not be matched to known colors"
        ));
    }

    // Reverse fills so bottom colors are at index 0
    for bottle in &mut bottles {
        bottle.fills.reverse();
    }

    // Save layout visualization for debugging
    let _ = imgcodecs::imwrite("layout.png", frame_display, &Default::default());
    let _ = imgcodecs::imwrite("layout-raw.png", frame_raw, &Default::default());

    Ok(bottles)
}
