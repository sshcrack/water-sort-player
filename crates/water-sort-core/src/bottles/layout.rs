use crate::{
    bottles::has_failed_level,
    constants::{color_distance_sq, vec3_from_hex},
    position::Pos,
};
use log::debug;
use opencv::core::{Mat, MatTraitConst};

use super::{LayerSample, best_matching_surrounding_pixel, classify_bottle_layer};

#[derive(Debug, Clone, Copy)]
struct LayoutFit {
    score: i32,
    unknown_bottles: usize,
    empty_bottles: usize,
}

/// Macro to create bottle layouts from a declarative specification.
///
/// # Parameters
/// - `$name`: Layout name (as string literal)
/// - `$layer_spacing`: Vertical pixel spacing between color layers within a bottle
/// - `$layer_count`: Number of color layers per bottle (typically 4)
/// - `$(row_specs)+`: One or more row specifications in the format: `(start_pos, spacing, count)`
///   - `start_pos`: `Pos(x, y)` - top-left position of the first bottle in the row
///   - `spacing`: `Pos(dx, dy)` - offset between consecutive bottles in the row
///   - `count`: number of bottles in this row
///
/// # Example
/// ```ignore
/// bottle_layout!(
///     "my-layout",
///     35,  // layer_spacing
///     4,   // layer_count
///     (Pos(41, 223), Pos(69, 0), 5),   // First row: 5 bottles, 69px apart
///     (Pos(41, 440), Pos(69, 0), 5),   // Second row: 5 bottles, 69px apart
/// )
/// ```
macro_rules! bottle_layout {
    ($name:expr, $layer_spacing:expr, $layer_count:expr, $(($start:expr, $spacing:expr, $count:expr)),+ $(,)?) => {{
        let mut positions = Vec::new();
        $(
            for col in 0..$count {
                let base_pos = Pos(
                    $start.0 + col * $spacing.0 + crate::constants::X_MEASURE_OFFSET,
                    $start.1 + col * $spacing.1 + crate::constants::Y_MEASURE_OFFSET,
                );
                positions.push(BottlePosition::vertical(base_pos, $layer_spacing, $layer_count));
            }
        )+
        BottleLayout::new($name.to_string(), positions)
    }};
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct BottleLayout {
    pub name: String,
    pub positions: Vec<BottlePosition>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BottlePosition {
    /// Starting position for bottle detection (top layer)
    pub base_pos: Pos,
    /// Offsets for each color layer (from top to bottom)
    pub layer_offsets: Vec<Pos>,
}

impl BottleLayout {
    fn bottle_looks_like_hidden_curtain(
        image: &Mat,
        layout: &BottleLayout,
        bottle_idx: usize,
    ) -> anyhow::Result<bool> {
        let Some(top_pos) = layout.get_sample_position(bottle_idx, 0) else {
            return Ok(false);
        };
        let Some(bottom_pos) = layout.get_sample_position(bottle_idx, 3) else {
            return Ok(false);
        };

        let curtain_reference = vec3_from_hex("#268072");
        let min_x = (top_pos.0 - 22).max(0);
        let max_x = (top_pos.0 + 22).min(image.cols() - 1);
        let min_y = (top_pos.1 - 8).max(0);
        let max_y = (bottom_pos.1 + 32).min(image.rows() - 1);

        let mut total_samples = 0usize;
        let mut curtain_like_samples = 0usize;

        for y in (min_y..=max_y).step_by(4) {
            for x in (min_x..=max_x).step_by(4) {
                let pixel = image.at_2d::<opencv::core::Vec3b>(y, x)?;
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

        let ratio = curtain_like_samples as f32 / total_samples as f32;
        Ok(ratio >= 0.45)
    }

    /// Create a new layout with the given name and positions
    pub fn new(name: String, positions: Vec<BottlePosition>) -> Self {
        Self { name, positions }
    }

    /// Get the number of bottles in this layout
    pub fn bottle_count(&self) -> usize {
        self.positions.len()
    }

    /// Get the position for sampling a specific color layer in a bottle
    pub fn get_sample_position(&self, bottle_index: usize, layer_index: usize) -> Option<Pos> {
        let bottle_pos = self.positions.get(bottle_index)?;
        let layer_offset = bottle_pos.layer_offsets.get(layer_index)?;

        Some(Pos(
            bottle_pos.base_pos.0 + layer_offset.0,
            bottle_pos.base_pos.1 + layer_offset.1,
        ))
    }

    /// Get a safe click position within a bottle (use middle layer by default)
    pub fn get_click_position(&self, bottle_index: usize) -> Option<Pos> {
        let bottle_pos = self.positions.get(bottle_index)?;
        if bottle_pos.layer_offsets.is_empty() {
            return Some(bottle_pos.base_pos);
        }

        let mid_index = bottle_pos.layer_offsets.len() / 2;
        let layer_offset = bottle_pos.layer_offsets.get(mid_index)?;
        Some(Pos(
            bottle_pos.base_pos.0 + layer_offset.0,
            bottle_pos.base_pos.1 + layer_offset.1,
        ))
    }

    /// Attempt to automatically detect the best layout for an image
    pub fn detect_layout(image: &Mat) -> anyhow::Result<Self> {
        let layouts = Self::get_layouts();

        let mut best_layout = layouts[0].clone();
        let mut best_score = i32::MIN;
        let mut best_raw_score = i32::MIN;
        let mut best_unknown_bottles = usize::MAX;
        let mut best_empty_bottles = usize::MAX;
        let has_failed_level = has_failed_level(image)?;

        for layout in layouts {
            let fit = Self::score_layout_fit(image, &layout, has_failed_level)?;
            let raw_score = fit.score;
            let normalized_score = raw_score.saturating_mul(100) / layout.bottle_count() as i32;
            let effective_score = if has_failed_level {
                normalized_score
            } else {
                raw_score
            };
            debug!(
                "Layout '{}' fit score: {} (normalized: {}, effective: {}, unknown_bottles: {}, empty_bottles: {})",
                layout.name,
                raw_score,
                normalized_score,
                effective_score,
                fit.unknown_bottles,
                fit.empty_bottles,
            );
            if effective_score > best_score
                || (effective_score == best_score && raw_score > best_raw_score)
                || (effective_score == best_score
                    && raw_score == best_raw_score
                    && fit.unknown_bottles < best_unknown_bottles)
                || (effective_score == best_score
                    && raw_score == best_raw_score
                    && fit.unknown_bottles == best_unknown_bottles
                    && fit.empty_bottles < best_empty_bottles)
                || (effective_score == best_score
                    && raw_score == best_raw_score
                    && fit.unknown_bottles == best_unknown_bottles
                    && fit.empty_bottles == best_empty_bottles
                    && layout.bottle_count() > best_layout.bottle_count())
            {
                best_score = effective_score;
                best_raw_score = raw_score;
                best_unknown_bottles = fit.unknown_bottles;
                best_empty_bottles = fit.empty_bottles;
                best_layout = layout;
            }
        }

        Ok(best_layout)
    }

    /// Score how well a layout fits an image based on detected bottles
    fn score_layout_fit(
        image: &Mat,
        layout: &BottleLayout,
        has_failed_level: bool,
    ) -> anyhow::Result<LayoutFit> {
        let mut score = 0;
        let mut unknown_bottles = 0usize;
        let mut empty_bottles = 0usize;

        for bottle_idx in 0..layout.bottle_count() {
            let mut color_amount = 0;
            let mut bottle_is_valid = true;
            let mut saw_unknown = false;

            // A valid bottle is filled contiguously from the top down.
            for layer_idx in 0..layout.positions[bottle_idx].layer_offsets.len() {
                if let Some(sample_pos) = layout.get_sample_position(bottle_idx, layer_idx) {
                    let x = sample_pos.0;
                    let y = sample_pos.1;

                    // Check if coordinates are within image bounds
                    if y >= 0 && y < image.rows() && x >= 0 && x < image.cols() {
                        let pixel = best_matching_surrounding_pixel(image, x, y, 4)?;

                        match classify_bottle_layer(pixel, has_failed_level) {
                            LayerSample::Empty => {
                                if color_amount > 0 {
                                    bottle_is_valid = false;
                                    break;
                                }
                            }
                            LayerSample::Color(_) => {
                                color_amount += 1;
                            }
                            LayerSample::Unknown => {
                                saw_unknown = true;
                                break;
                            }
                        }
                    }
                }
            }
            let looks_hidden_curtain =
                Self::bottle_looks_like_hidden_curtain(image, layout, bottle_idx)?;

            if looks_hidden_curtain {
                // Reward potential hidden-bottle curtains so hidden levels pick the right layout.
                score += 3;
            } else if bottle_is_valid && color_amount > 0 {
                score += color_amount;
                if saw_unknown {
                    unknown_bottles += 1;
                }
            } else if saw_unknown && color_amount == 0 {
                score -= 2;
                unknown_bottles += 1;
            } else if !bottle_is_valid {
                // Penalize layouts that place many sampling points on non-bottle content.
                score -= 2;
                if saw_unknown {
                    unknown_bottles += 1;
                }
            } else if color_amount == 0 {
                // Penalize layouts that miss bottles entirely.
                score -= 1;
                empty_bottles += 1;
            }
        }

        Ok(LayoutFit {
            score,
            unknown_bottles,
            empty_bottles,
        })
    }
}

impl BottlePosition {
    /// Create a new bottle position
    pub fn new(base_pos: Pos, layer_offsets: Vec<Pos>) -> Self {
        Self {
            base_pos,
            layer_offsets,
        }
    }

    /// Create a standard vertical bottle with equally spaced layers
    pub fn vertical(base_pos: Pos, layer_spacing: i32, layer_count: usize) -> Self {
        let layer_offsets = (0..layer_count)
            .map(|i| Pos(0, i as i32 * layer_spacing))
            .collect();

        Self::new(base_pos, layer_offsets)
    }
}

include!(concat!(env!("OUT_DIR"), "/generated_layouts.rs"));
