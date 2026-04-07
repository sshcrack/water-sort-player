use opencv::{core::Mat, imgproc};

use crate::{
    bottles::{Bottle, BottleLayout},
    constants::BottleColor,
};

const BOTTLE_CAPACITY: usize = 4;
pub fn draw_revealed_fill_markers(
    frame_display: &mut Mat,
    layout: &BottleLayout,
    max_revealed_bottle_state: &[Bottle],
) -> anyhow::Result<()> {
    for (bottle_index, bottle) in max_revealed_bottle_state.iter().enumerate() {
        for (fill_index, color) in bottle.get_fills().iter().enumerate().take(BOTTLE_CAPACITY) {
            if *color == BottleColor::Mystery {
                continue;
            }

            // Fill indices are bottom->top while sampling layers are top->bottom.
            let layer_index = (BOTTLE_CAPACITY - 1).saturating_sub(fill_index);
            if let Some(sample_pos) = layout.get_sample_position(bottle_index, layer_index) {
                imgproc::rectangle(
                    frame_display,
                    opencv::core::Rect::new(sample_pos.0 - 10, sample_pos.1 - 10, 20, 20),
                    color.to_pixel_value().into(),
                    2,
                    imgproc::LINE_8,
                    0,
                )?;
            }
        }
    }

    Ok(())
}
