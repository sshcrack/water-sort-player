use opencv::{
    core::{Mat, MatTraitConst, Rect, Scalar, Vec3b},
    imgproc,
};

#[cfg(test)]
mod tests;

use crate::constants::{
    BOTTLE_SPACING, BottleColor, COLOR_CHECK_OFFSET, FIRST_ROW_START_POS, SECOND_ROW_OFFSET,
};

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
