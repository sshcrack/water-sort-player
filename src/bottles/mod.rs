use opencv::{
    core::{Mat, MatTraitConst, Rect, Scalar, Vec3b},
    imgproc,
};

#[cfg(test)]
mod tests;

use crate::constants::{
    BOTTLE_SPACING, BottleColor, COLOR_CHECK_OFFSET, FIRST_ROW_START_POS, SECOND_ROW_OFFSET,
    is_color_within_tolerance, EMPTY_COLOR, COLOR_VALUES,
};

/// Represents confidence in a color detection (0.0 to 1.0)
#[derive(Debug, Clone, Copy)]
struct ColorConfidence {
    color: Option<BottleColor>,
    confidence: f32,
    avg_tolerance_used: u8,
}

/// Configuration for bottle detection layouts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottleLayout {
    Standard10, // 2 rows x 5 columns
    Extended12, // 3 rows x 4 columns
}

impl BottleLayout {
    pub fn rows(&self) -> usize {
        match self {
            Self::Standard10 => 2,
            Self::Extended12 => 3,
        }
    }

    pub fn cols(&self) -> usize {
        match self {
            Self::Standard10 => 5,
            Self::Extended12 => 4,
        }
    }

    pub fn total_bottles(&self) -> usize {
        self.rows() * self.cols()
    }
}

impl Default for BottleLayout {
    fn default() -> Self {
        Self::Standard10
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Bottle {
    // Last element is the top color, first element is the bottom color
    fills: Vec<BottleColor>,
}

const ROW_COUNT: usize = 2;
const COL_COUNT: usize = 5;

const FULL_BOTTLE_COUNT: usize = 4;

impl Bottle {
    #[cfg(test)]
    pub fn from_fills(fills: Vec<BottleColor>) -> Self {
        Bottle { fills }
    }

    pub fn get_fills(&self) -> &Vec<BottleColor> {
        &self.fills
    }

    pub fn get_top_fill(&self) -> Option<(usize, BottleColor)> {
        let mut last_fill = None;
        let mut amount = 0;

        for (i, color) in self.fills.iter().rev().enumerate() {
            if i == 0 {
                last_fill = Some(color);
                amount = 1;
            } else if Some(color) == last_fill {
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
        if self.get_fill_count() != FULL_BOTTLE_COUNT {
            return false;
        }

        let first_color = self.fills[0];
        self.fills.iter().all(|&color| color == first_color)
    }

    pub fn can_fill_from(&self, other: &Bottle) -> bool {
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
            self.fills.push(source_top_color);
            source.fills.pop();
        }
    }
}

impl Default for Bottle {
    fn default() -> Self {
        Bottle { fills: Vec::new() }
    }
}

/// Detects color by sampling a region around a point instead of a single pixel.
/// Returns confidence score and detected color.
fn detect_color_with_region(
    frame: &Mat,
    x: i32,
    y: i32,
    sample_radius: i32,
) -> anyhow::Result<ColorConfidence> {
    let height = frame.rows() as i32;
    let width = frame.cols() as i32;

    // Clamp bounds to image dimensions
    let x_min = (x - sample_radius).max(0).min(width - 1);
    let x_max = (x + sample_radius).min(width - 1).max(x_min);
    let y_min = (y - sample_radius).max(0).min(height - 1);
    let y_max = (y + sample_radius).min(height - 1).max(y_min);

    let mut color_scores: Vec<(Option<BottleColor>, f32, u8)> = vec![(None, 0.0, 255)];

    // Sample pixels in the region
    for py in y_min..=y_max {
        for px in x_min..=x_max {
            let pixel = frame.at_2d::<Vec3b>(py, px)?;

            // Check if it's an empty pixel
            if is_color_within_tolerance(&pixel, &*EMPTY_COLOR, 30) {
                color_scores[0].1 += 1.0;
                continue;
            }

            // Try to match against known colors with adaptive tolerance
            for (color, target_pixel) in COLOR_VALUES.iter() {
                let b_diff = (pixel[0] as i16 - target_pixel[0] as i16).abs() as u8;
                let g_diff = (pixel[1] as i16 - target_pixel[1] as i16).abs() as u8;
                let r_diff = (pixel[2] as i16 - target_pixel[2] as i16).abs() as u8;

                let max_diff = b_diff.max(g_diff).max(r_diff);

                if max_diff <= 50 {
                    // Found a match, update or insert score
                    let score = 100.0 - (max_diff as f32);

                    if let Some(entry) = color_scores.iter_mut().find(|(c, _, _)| c == &Some(*color)) {
                        entry.1 += score;
                        entry.2 = entry.2.min(max_diff);
                    } else {
                        color_scores.push((Some(*color), score, max_diff));
                    }
                }
            }
        }
    }

    // Find the best match
    let best = color_scores.iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap();

    let total_samples = ((x_max - x_min + 1) * (y_max - y_min + 1)) as f32;
    let confidence = best.1 / (total_samples * 100.0);

    Ok(ColorConfidence {
        color: best.0,
        confidence: confidence.min(1.0),
        avg_tolerance_used: best.2,
    })
}

/// Validates detected bottles against game rules
fn validate_detected_bottles(bottles: &[Bottle]) -> bool {
    let mut total_fills = 0;

    for bottle in bottles {
        if bottle.get_fill_count() > FULL_BOTTLE_COUNT {
            return false; // Bottle has too many fills
        }
        total_fills += bottle.get_fill_count();
    }

    // Calculate expected total fills: each color should appear 4 times maximum
    let max_expected_fills = bottles.len() * FULL_BOTTLE_COUNT;

    // The total should be reasonable (allow 1-2 extra due to detection errors)
    total_fills <= max_expected_fills + 2
}

pub fn detect_and_draw_bottles_improved(
    frame_raw: &Mat,
    frame_display: &mut Mat,
    layout: BottleLayout,
) -> anyhow::Result<Vec<Bottle>> {
    let mut bottles = Vec::new();

    for _ in 0..layout.total_bottles() {
        bottles.push(Bottle::default());
    }

    for row in 0..layout.rows() {
        for col in 0..layout.cols() {
            let mut x = FIRST_ROW_START_POS.0 + col as i32 * BOTTLE_SPACING.0;
            let mut y = FIRST_ROW_START_POS.1 + row as i32 * BOTTLE_SPACING.1;

            if row == 1 && layout == BottleLayout::Standard10 {
                x += SECOND_ROW_OFFSET.0;
                y += SECOND_ROW_OFFSET.1;
            }

            for _level in 0..4 {
                // Draw sampling rectangle
                imgproc::rectangle(
                    frame_display,
                    Rect::new(x - 20, y - 20, 40, 40),
                    Scalar::new(0.0, 255.0, 0.0, 255.0),
                    2,
                    imgproc::LINE_8,
                    0,
                )
                .unwrap();

                // Use region-based detection with sample radius of 5-8 pixels
                let color_conf = detect_color_with_region(frame_raw, x, y, 5)?;

                if let Some(color) = color_conf.color {
                    // Only accept colors with reasonable confidence
                    if color_conf.confidence > 0.2 {
                        imgproc::rectangle(
                            frame_display,
                            Rect::new(x - 5, y - 5, 10, 10),
                            color.to_pixel_value().into(),
                            2,
                            imgproc::LINE_8,
                            0,
                        )
                        .unwrap();
                        bottles[row * layout.cols() + col].fills.push(color);
                    } else {
                        // Low confidence - mark as unknown
                        imgproc::rectangle(
                            frame_display,
                            Rect::new(x - 5, y - 5, 10, 10),
                            Scalar::new(128.0, 128.0, 128.0, 255.0),
                            2,
                            imgproc::LINE_8,
                            0,
                        )
                        .unwrap();
                    }
                } else {
                    // Empty or undetected
                    imgproc::rectangle(
                        frame_display,
                        Rect::new(x - 5, y - 5, 10, 10),
                        Scalar::new(255.0, 255.0, 255.0, 255.0),
                        2,
                        imgproc::LINE_8,
                        0,
                    )
                    .unwrap();
                }

                x += COLOR_CHECK_OFFSET.0;
                y += COLOR_CHECK_OFFSET.1;
            }
        }
    }

    for ele in &mut bottles {
        ele.fills.reverse();
    }

    // Validate detected bottles
    if validate_detected_bottles(&bottles) {
        Ok(bottles)
    } else {
        // Return fallback to original detection if validation fails
        detect_and_draw_bottles(frame_raw, frame_display)
    }
}

pub fn detect_and_draw_bottles_with_layout(
    frame_raw: &Mat,
    frame_display: &mut Mat,
    layout: BottleLayout,
) -> anyhow::Result<Vec<Bottle>> {
    detect_and_draw_bottles_improved(frame_raw, frame_display, layout)
}

pub fn detect_and_draw_bottles(
    frame_raw: &Mat,
    frame_display: &mut Mat,
) -> anyhow::Result<Vec<Bottle>> {
    let mut bottles = Vec::new();

    for _ in 0..ROW_COUNT * COL_COUNT {
        bottles.push(Bottle::default());
    }

    for row in 0..ROW_COUNT {
        for col in 0..COL_COUNT {
            let mut x = FIRST_ROW_START_POS.0 + col as i32 * BOTTLE_SPACING.0;
            let mut y = FIRST_ROW_START_POS.1 + row as i32 * BOTTLE_SPACING.1;

            if row == 1 {
                x += SECOND_ROW_OFFSET.0;
                y += SECOND_ROW_OFFSET.1;
            }

            for _ in 0..4 {
                imgproc::rectangle(
                    frame_display,
                    Rect::new(x - 20, y - 20, 40, 40),
                    Scalar::new(0.0, 255.0, 0.0, 255.0),
                    2,
                    imgproc::LINE_8,
                    0,
                )
                .unwrap();

                let pixel = frame_raw.at_2d::<Vec3b>(y, x)?;
                if BottleColor::is_empty_pixel(pixel) {
                    imgproc::rectangle(
                        frame_display,
                        Rect::new(x - 5, y - 5, 10, 10),
                        Scalar::new(255.0, 255.0, 255.0, 255.0),
                        2,
                        imgproc::LINE_8,
                        0,
                    )
                    .unwrap();
                } else if let Some(color) = BottleColor::from_pixel_value(*pixel) {
                    imgproc::rectangle(
                        frame_display,
                        Rect::new(x - 5, y - 5, 10, 10),
                        color.to_pixel_value().into(),
                        2,
                        imgproc::LINE_8,
                        0,
                    )
                    .unwrap();
                    bottles[row * COL_COUNT + col].fills.push(color);
                } else {
                    imgproc::rectangle(
                        frame_display,
                        Rect::new(x - 5, y - 5, 10, 10),
                        Scalar::new(0.0, 0.0, 0.0, 255.0),
                        2,
                        imgproc::LINE_8,
                        0,
                    )
                    .unwrap();
                }

                x += COLOR_CHECK_OFFSET.0;
                y += COLOR_CHECK_OFFSET.1;
            }
        }
    }

    for ele in &mut bottles {
        ele.fills.reverse();
    }

    Ok(bottles)
}
