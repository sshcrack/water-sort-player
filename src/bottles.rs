use opencv::{
    core::{Mat, Rect, Scalar},
    imgproc,
};

use crate::constants::{BOTTLE_SPACING, Color, FIRST_ROW_START_POS, SECOND_ROW_OFFSET};

pub fn detect_and_draw_bottles(frame: &mut Mat) -> [[Vec<Color>; 5]; 2] {
    let mut bottles: [[Vec<Color>; 5]; 2] =
        std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()));

    for row in 0..2 {
        for col in 0..5 {
            let mut x = FIRST_ROW_START_POS.0 + col as i32 * BOTTLE_SPACING.0;
            let mut y = FIRST_ROW_START_POS.1 + row as i32 * BOTTLE_SPACING.1;

            if row == 1 {
                x += SECOND_ROW_OFFSET.0;
                y += SECOND_ROW_OFFSET.1;
            }

            bottles[row][col].push(Color::Red);

            imgproc::rectangle(
                frame,
                Rect::new(x - 20, y - 20, 40, 40),
                Scalar::new(0.0, 255.0, 0.0, 255.0),
                2,
                imgproc::LINE_8,
                0,
            )
            .unwrap();
        }
    }

    bottles
}