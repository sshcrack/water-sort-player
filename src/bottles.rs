use opencv::{
    core::{Mat, MatTraitConst, Rect, Scalar, Vec3b},
    imgproc,
};

use crate::constants::{
    BOTTLE_SPACING, COLOR_CHECK_OFFSET, BottleState, FIRST_ROW_START_POS, SECOND_ROW_OFFSET,
};

pub fn detect_and_draw_bottles(
    frame_raw: &Mat,
    frame_display: &mut Mat,
) -> anyhow::Result<[[Vec<BottleState>; 5]; 2]> {
    let mut bottles: [[Vec<BottleState>; 5]; 2] =
        std::array::from_fn(|_| std::array::from_fn(|_| Vec::new()));

    for row in 0..2 {
        for col in 0..5 {
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
                if let Some(color) = BottleState::from_pixel_value(*pixel) {
                    imgproc::rectangle(
                        frame_display,
                        Rect::new(x - 5, y - 5, 10, 10),
                        color.to_pixel_value().into(),
                        2,
                        imgproc::LINE_8,
                        0,
                    )
                    .unwrap();
                    bottles[row][col].push(color);
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

    Ok(bottles)
}
