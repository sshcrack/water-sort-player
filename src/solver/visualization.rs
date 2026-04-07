use opencv::{core::Mat, imgproc};

use crate::{
    bottles::{Bottle, BottleLayout},
    constants::BottleColor,
};

use super::Move;

const BACKGROUND: u32 = 0x10151c;
const PANEL: u32 = 0x151b24;
const PANEL_ACCENT: u32 = 0x223041;
const OUTLINE: u32 = 0x91a4b6;
const EMPTY_SLOT: u32 = 0x202734;
const SOLVED_OUTLINE: u32 = 0x46d17c;
const SOURCE_OUTLINE: u32 = 0xf15a5a;
const DEST_OUTLINE: u32 = 0x4fc7ff;
const STEP_BAR: u32 = 0xf5c542;

const GRID_COLS: usize = 5;
const GRID_ROWS: usize = 2;
const BOTTLE_CAPACITY: usize = 4;
const BOTTLE_WIDTH: usize = 54;
const BOTTLE_HEIGHT: usize = 118;
const BOTTLE_COL_GAP: usize = 14;
const BOTTLE_ROW_GAP: usize = 18;
const TOP_MARGIN: usize = 28;

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn bottle_color_to_rgb(color: BottleColor) -> u32 {
    let pixel = color.to_pixel_value();
    rgb(pixel[2], pixel[1], pixel[0])
}

fn set_pixel(buffer: &mut [u32], width: usize, height: usize, x: i32, y: i32, color: u32) {
    if x < 0 || y < 0 {
        return;
    }

    let x = x as usize;
    let y = y as usize;
    if x >= width || y >= height {
        return;
    }

    buffer[y * width + x] = color;
}

fn fill_rect(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    color: u32,
) {
    let max_x = (x + rect_width).min(width);
    let max_y = (y + rect_height).min(height);

    for row in y..max_y {
        let start = row * width + x.min(width);
        let end = row * width + max_x;
        for pixel in &mut buffer[start..end] {
            *pixel = color;
        }
    }
}

fn draw_rect_outline(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    thickness: usize,
    color: u32,
) {
    for offset in 0..thickness {
        let left = x + offset;
        let top = y + offset;
        let right = x + rect_width.saturating_sub(1 + offset);
        let bottom = y + rect_height.saturating_sub(1 + offset);

        if left >= width || top >= height {
            continue;
        }

        for x_pos in left..=right.min(width.saturating_sub(1)) {
            set_pixel(buffer, width, height, x_pos as i32, top as i32, color);
            set_pixel(buffer, width, height, x_pos as i32, bottom as i32, color);
        }

        for y_pos in top..=bottom.min(height.saturating_sub(1)) {
            set_pixel(buffer, width, height, left as i32, y_pos as i32, color);
            set_pixel(buffer, width, height, right as i32, y_pos as i32, color);
        }
    }
}

fn draw_line(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    mut x0: i32,
    mut y0: i32,
    x1: i32,
    y1: i32,
    color: u32,
) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        set_pixel(buffer, width, height, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }

        let twice_err = 2 * err;
        if twice_err >= dy {
            err += dy;
            x0 += sx;
        }
        if twice_err <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn draw_background(buffer: &mut [u32], width: usize, height: usize) {
    fill_rect(buffer, width, height, 0, 0, width, height, BACKGROUND);

    for row in (0..height).step_by(8) {
        let shade = if row % 16 == 0 { PANEL } else { BACKGROUND };
        fill_rect(buffer, width, height, 0, row, width, 1, shade);
    }

    fill_rect(buffer, width, height, 0, 0, width, 3, PANEL_ACCENT);
}

#[allow(dead_code)]
pub fn render_solver_view(
    width: usize,
    height: usize,
    bottles: &[Bottle],
    active_move: Option<Move>,
) -> Vec<u32> {
    let mut buffer = vec![BACKGROUND; width * height];
    draw_background(&mut buffer, width, height);

    let grid_width = GRID_COLS * BOTTLE_WIDTH + (GRID_COLS - 1) * BOTTLE_COL_GAP;
    let grid_height = GRID_ROWS * BOTTLE_HEIGHT + (GRID_ROWS - 1) * BOTTLE_ROW_GAP;
    let origin_x = width.saturating_sub(grid_width) / 2;
    let origin_y = TOP_MARGIN + (height.saturating_sub(TOP_MARGIN + grid_height)) / 2;

    if let Some(move_to_show) = active_move {
        let source_row = move_to_show.0 / GRID_COLS;
        let source_col = move_to_show.0 % GRID_COLS;
        let destination_row = move_to_show.1 / GRID_COLS;
        let destination_col = move_to_show.1 % GRID_COLS;

        let source_x = origin_x + source_col * (BOTTLE_WIDTH + BOTTLE_COL_GAP) + BOTTLE_WIDTH / 2;
        let source_y = origin_y + source_row * (BOTTLE_HEIGHT + BOTTLE_ROW_GAP) + BOTTLE_HEIGHT / 2;
        let destination_x =
            origin_x + destination_col * (BOTTLE_WIDTH + BOTTLE_COL_GAP) + BOTTLE_WIDTH / 2;
        let destination_y =
            origin_y + destination_row * (BOTTLE_HEIGHT + BOTTLE_ROW_GAP) + BOTTLE_HEIGHT / 2;

        draw_line(
            &mut buffer,
            width,
            height,
            source_x as i32,
            source_y as i32,
            destination_x as i32,
            destination_y as i32,
            STEP_BAR,
        );
    }

    for row in 0..GRID_ROWS {
        for col in 0..GRID_COLS {
            let bottle_index = row * GRID_COLS + col;
            if bottle_index >= bottles.len() {
                continue;
            }

            let bottle = &bottles[bottle_index];
            let x = origin_x + col * (BOTTLE_WIDTH + BOTTLE_COL_GAP);
            let y = origin_y + row * (BOTTLE_HEIGHT + BOTTLE_ROW_GAP);
            let interior_x = x + 4;
            let interior_y = y + 4;
            let interior_width = BOTTLE_WIDTH.saturating_sub(8);
            let interior_height = BOTTLE_HEIGHT.saturating_sub(8);

            fill_rect(
                &mut buffer,
                width,
                height,
                x,
                y,
                BOTTLE_WIDTH,
                BOTTLE_HEIGHT,
                PANEL,
            );
            draw_rect_outline(
                &mut buffer,
                width,
                height,
                x,
                y,
                BOTTLE_WIDTH,
                BOTTLE_HEIGHT,
                2,
                if bottle.is_solved() {
                    SOLVED_OUTLINE
                } else if bottle.is_empty() {
                    EMPTY_SLOT
                } else {
                    OUTLINE
                },
            );

            let segment_height = interior_height / BOTTLE_CAPACITY;
            for slot in 0..BOTTLE_CAPACITY {
                let slot_y = interior_y + interior_height - (slot + 1) * segment_height + 2;
                let slot_height = segment_height.saturating_sub(4);
                fill_rect(
                    &mut buffer,
                    width,
                    height,
                    interior_x,
                    slot_y,
                    interior_width,
                    slot_height,
                    rgb(34, 42, 56),
                );
            }

            for (slot_index, fill) in bottle.get_fills().iter().enumerate() {
                let slot_y = interior_y + interior_height - (slot_index + 1) * segment_height + 2;
                let slot_height = segment_height.saturating_sub(4);
                fill_rect(
                    &mut buffer,
                    width,
                    height,
                    interior_x,
                    slot_y,
                    interior_width,
                    slot_height,
                    bottle_color_to_rgb(*fill),
                );
            }

            if let Some(move_to_show) = active_move {
                if move_to_show.0 == bottle_index {
                    draw_rect_outline(
                        &mut buffer,
                        width,
                        height,
                        x.saturating_sub(2),
                        y.saturating_sub(2),
                        BOTTLE_WIDTH + 4,
                        BOTTLE_HEIGHT + 4,
                        2,
                        SOURCE_OUTLINE,
                    );
                }

                if move_to_show.1 == bottle_index {
                    draw_rect_outline(
                        &mut buffer,
                        width,
                        height,
                        x.saturating_sub(2),
                        y.saturating_sub(2),
                        BOTTLE_WIDTH + 4,
                        BOTTLE_HEIGHT + 4,
                        2,
                        DEST_OUTLINE,
                    );
                }
            }
        }
    }

    buffer
}

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
