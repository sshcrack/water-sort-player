use crate::solver::Move;

const OVERLAY_PANEL_COLOR: u32 = 0x10141a;
const OVERLAY_BORDER_COLOR: u32 = 0x39414a;
const MOVE_DONE_COLOR: u32 = 0x3fd46d;
const MOVE_CURRENT_COLOR: u32 = 0xffc34d;
const MOVE_PENDING_COLOR: u32 = 0x525f6d;
const PROGRESS_BG_COLOR: u32 = 0x2c333b;
const PROGRESS_FILL_COLOR: u32 = 0x3ea6ff;

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
        for col in x..max_x {
            buffer[row * width + col] = color;
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
    if rect_width == 0 || rect_height == 0 {
        return;
    }

    for offset in 0..thickness {
        let left = x + offset;
        let top = y + offset;
        if left >= width || top >= height {
            continue;
        }

        let right = (x + rect_width).saturating_sub(1 + offset).min(width - 1);
        let bottom = (y + rect_height).saturating_sub(1 + offset).min(height - 1);

        for col in left..=right {
            buffer[top * width + col] = color;
            buffer[bottom * width + col] = color;
        }

        for row in top..=bottom {
            buffer[row * width + left] = color;
            buffer[row * width + right] = color;
        }
    }
}

fn bottle_index_to_color(index: usize) -> u32 {
    const PALETTE: [u32; 10] = [
        0xff6b6b, 0x4dabf7, 0xffd43b, 0x69db7c, 0xf783ac, 0x94d82d, 0xff922b, 0x66d9e8, 0xb197fc,
        0xff8787,
    ];

    PALETTE[index % PALETTE.len()]
}

pub fn draw_move_overlay(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    planned_moves: &[Move],
    performed_moves: usize,
    active_move: Option<Move>,
) {
    let panel_x = 12;
    let panel_y = 12;
    let panel_width = width.saturating_sub(24).min(420);
    let panel_height = 78;

    fill_rect(
        buffer,
        width,
        height,
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        OVERLAY_PANEL_COLOR,
    );
    draw_rect_outline(
        buffer,
        width,
        height,
        panel_x,
        panel_y,
        panel_width,
        panel_height,
        2,
        OVERLAY_BORDER_COLOR,
    );

    let progress_outer_x = panel_x + 12;
    let progress_outer_y = panel_y + panel_height - 20;
    let progress_outer_w = panel_width.saturating_sub(24);
    let progress_outer_h = 10;

    fill_rect(
        buffer,
        width,
        height,
        progress_outer_x,
        progress_outer_y,
        progress_outer_w,
        progress_outer_h,
        PROGRESS_BG_COLOR,
    );
    draw_rect_outline(
        buffer,
        width,
        height,
        progress_outer_x,
        progress_outer_y,
        progress_outer_w,
        progress_outer_h,
        1,
        OVERLAY_BORDER_COLOR,
    );

    if !planned_moves.is_empty() {
        let progress = performed_moves.min(planned_moves.len());
        let fill_width = (progress_outer_w.saturating_sub(2) * progress) / planned_moves.len();
        fill_rect(
            buffer,
            width,
            height,
            progress_outer_x + 1,
            progress_outer_y + 1,
            fill_width,
            progress_outer_h.saturating_sub(2),
            PROGRESS_FILL_COLOR,
        );
    }

    let slots = planned_moves.len().max(1).min(24);
    let slot_width = (panel_width.saturating_sub(24)) / slots;
    let moves_preview = planned_moves.len().min(24);
    for (index, m) in planned_moves.iter().take(moves_preview).enumerate() {
        let x = panel_x + 12 + index * slot_width;
        let y = panel_y + 14;
        let w = slot_width.saturating_sub(3).max(4);
        let h = 28;

        let status_color = if index < performed_moves {
            MOVE_DONE_COLOR
        } else if index == performed_moves {
            MOVE_CURRENT_COLOR
        } else {
            MOVE_PENDING_COLOR
        };

        fill_rect(buffer, width, height, x, y, w, h, status_color);
        draw_rect_outline(buffer, width, height, x, y, w, h, 1, OVERLAY_BORDER_COLOR);

        let source_color = bottle_index_to_color(m.source_index());
        let destination_color = bottle_index_to_color(m.destination_index());
        fill_rect(
            buffer,
            width,
            height,
            x + 2,
            y + 2,
            w.saturating_sub(4),
            9,
            source_color,
        );
        fill_rect(
            buffer,
            width,
            height,
            x + 2,
            y + h.saturating_sub(11),
            w.saturating_sub(4),
            9,
            destination_color,
        );
    }

    if planned_moves.len() > 24 {
        let marker_x = panel_x + panel_width.saturating_sub(16);
        let marker_y = panel_y + 14;
        fill_rect(
            buffer,
            width,
            height,
            marker_x,
            marker_y,
            4,
            28,
            MOVE_PENDING_COLOR,
        );
    }

    if let Some(m) = active_move {
        let active_width = 36;
        let active_height = 10;
        let active_x = panel_x + panel_width.saturating_sub(active_width + 12);
        let active_y = panel_y + panel_height.saturating_sub(active_height + 28);
        fill_rect(
            buffer,
            width,
            height,
            active_x,
            active_y,
            active_width,
            active_height,
            MOVE_CURRENT_COLOR,
        );

        fill_rect(
            buffer,
            width,
            height,
            active_x + 2,
            active_y + 2,
            active_width / 2 - 3,
            active_height.saturating_sub(4),
            bottle_index_to_color(m.source_index()),
        );
        fill_rect(
            buffer,
            width,
            height,
            active_x + active_width / 2 + 1,
            active_y + 2,
            active_width / 2 - 3,
            active_height.saturating_sub(4),
            bottle_index_to_color(m.destination_index()),
        );
    }
}
