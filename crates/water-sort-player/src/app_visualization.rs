use std::time::Duration;

use anyhow::Result;
use opencv::{
    core::{Mat, Point, Rect, Scalar},
    imgproc,
};
use water_sort_core::constants::scalar_from_hex;

use crate::bottles::{Bottle, BottleLayout};
use crate::solver::Move;

fn hud_background() -> Scalar {
    scalar_from_hex("#0e1216")
}

fn hud_border() -> Scalar {
    scalar_from_hex("#8092a0")
}

fn hud_title() -> Scalar {
    scalar_from_hex("#d7e4eb")
}

fn hud_text() -> Scalar {
    scalar_from_hex("#b8c4cd")
}

fn hud_muted() -> Scalar {
    scalar_from_hex("#7a8a94")
}

fn hud_accent() -> Scalar {
    scalar_from_hex("#2cacf2")
}

fn hud_progress_bg() -> Scalar {
    scalar_from_hex("#3e4952")
}

fn hud_progress_fill() -> Scalar {
    scalar_from_hex("#ffc54e")
}

fn detected_empty_fill() -> Scalar {
    scalar_from_hex("#3a4248")
}

fn detected_slot_border() -> Scalar {
    scalar_from_hex("#1c2024")
}

fn color_to_scalar(color: water_sort_core::constants::BottleColor) -> Scalar {
    let pixel = color.to_pixel_value();
    Scalar::new(pixel[0] as f64, pixel[1] as f64, pixel[2] as f64, 0.0)
}

#[cfg(feature = "solver-visualization")]
fn solver_preview_bg() -> Scalar {
    scalar_from_hex("#181c20")
}

#[cfg(feature = "solver-visualization")]
fn solver_preview_border() -> Scalar {
    scalar_from_hex("#667480")
}

#[cfg(feature = "solver-visualization")]
fn solver_empty_fill() -> Scalar {
    scalar_from_hex("#282e34")
}

#[cfg(feature = "solver-visualization")]
fn solver_source_highlight() -> Scalar {
    // Bright red (BGR format)
    scalar_from_hex("#ff0000")
}

#[cfg(feature = "solver-visualization")]
fn solver_destination_highlight() -> Scalar {
    // Bright cyan (BGR format)
    scalar_from_hex("#00ffff")
}

#[cfg(feature = "solver-visualization")]
fn solver_arrow_color() -> Scalar {
    // Bright yellow (BGR format)
    scalar_from_hex("#ffff00")
}

pub struct OverlaySnapshot<'a> {
    pub phase: String,
    pub detail: String,
    pub until_ready: Option<Duration>,
    pub discovery_hidden: Option<usize>,
    pub discovery_total_slots: Option<usize>,
    pub discovery_depth: Option<usize>,
    pub discovery_queue: Option<usize>,
    pub solve_moves: &'a [Move],
    pub solve_performed_moves: usize,
    #[cfg(feature = "solver-visualization")]
    pub solve_layout: Option<&'a BottleLayout>,
    #[cfg(feature = "solver-visualization")]
    pub solve_current_move_index: usize,
}

fn format_duration(duration: Duration) -> String {
    format!("{:.1}s", duration.as_secs_f32())
}

fn discovery_progress_ratio(hidden: usize, total_slots: usize) -> f32 {
    if total_slots == 0 {
        return 1.0;
    }

    let hidden = hidden.min(total_slots) as f32;
    1.0 - (hidden / total_slots as f32)
}

pub fn draw_state_hud(
    frame_display: &mut Mat,
    width: usize,
    snapshot: &OverlaySnapshot<'_>,
) -> Result<()> {
    let panel_width = 360usize.min(width.saturating_sub(24));
    let panel_height = 142;
    let panel_x = width.saturating_sub(panel_width + 12) as i32;
    let panel_y = 12;

    imgproc::rectangle(
        frame_display,
        Rect::new(panel_x, panel_y, panel_width as i32, panel_height),
        hud_background(),
        imgproc::FILLED,
        imgproc::LINE_8,
        0,
    )?;
    imgproc::rectangle(
        frame_display,
        Rect::new(panel_x, panel_y, panel_width as i32, panel_height),
        hud_border(),
        2,
        imgproc::LINE_8,
        0,
    )?;

    imgproc::put_text(
        frame_display,
        &format!("State: {}", snapshot.phase),
        Point::new(panel_x + 12, panel_y + 28),
        imgproc::FONT_HERSHEY_DUPLEX,
        0.65,
        hud_title(),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    imgproc::put_text(
        frame_display,
        &snapshot.detail,
        Point::new(panel_x + 12, panel_y + 52),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.55,
        hud_text(),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    let timing = snapshot
        .until_ready
        .map(format_duration)
        .unwrap_or_else(|| "ready".to_string());
    imgproc::put_text(
        frame_display,
        &format!("Next action: {}", timing),
        Point::new(panel_x + 12, panel_y + 74),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.5,
        hud_muted(),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    if let (Some(hidden), Some(total_slots)) =
        (snapshot.discovery_hidden, snapshot.discovery_total_slots)
    {
        let ratio = discovery_progress_ratio(hidden, total_slots);
        let bar_x = panel_x + 12;
        let bar_y = panel_y + 86;
        let bar_width = panel_width as i32 - 24;
        let bar_height = 12;

        imgproc::rectangle(
            frame_display,
            Rect::new(bar_x, bar_y, bar_width, bar_height),
            hud_progress_bg(),
            imgproc::FILLED,
            imgproc::LINE_8,
            0,
        )?;

        let fill_width = ((bar_width - 2) as f32 * ratio.clamp(0.0, 1.0)) as i32;
        if fill_width > 0 {
            imgproc::rectangle(
                frame_display,
                Rect::new(bar_x + 1, bar_y + 1, fill_width, bar_height - 2),
                hud_progress_fill(),
                imgproc::FILLED,
                imgproc::LINE_8,
                0,
            )?;
        }

        let depth = snapshot.discovery_depth.unwrap_or(0);
        let queue = snapshot.discovery_queue.unwrap_or(0);
        imgproc::put_text(
            frame_display,
            &format!(
                "Discovery: {} hidden | path {} | queue {}",
                hidden, depth, queue
            ),
            Point::new(panel_x + 12, panel_y + 116),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            hud_accent(),
            1,
            imgproc::LINE_AA,
            false,
        )?;
    } else {
        let solved_label = if snapshot.solve_moves.is_empty() {
            "Solve queue: none".to_string()
        } else {
            format!(
                "Solve progress: {}/{}",
                snapshot
                    .solve_performed_moves
                    .min(snapshot.solve_moves.len()),
                snapshot.solve_moves.len()
            )
        };

        imgproc::put_text(
            frame_display,
            &solved_label,
            Point::new(panel_x + 12, panel_y + 110),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            hud_accent(),
            1,
            imgproc::LINE_AA,
            false,
        )?;
    }

    #[cfg(feature = "solver-visualization")]
    draw_solver_move_indicators(frame_display, snapshot)?;

    Ok(())
}

pub fn draw_detected_bottles_overlay(
    frame_display: &mut Mat,
    layout: &BottleLayout,
    bottles: &[Bottle],
) -> Result<()> {
    let bottle_count = layout.bottle_count().min(bottles.len());

    for (bottle_idx, bottle) in bottles.iter().enumerate().take(bottle_count) {
        if let Some(water_sort_core::position::Pos(cx, cy)) = layout.get_click_position(bottle_idx)
        {
            imgproc::circle(
                frame_display,
                Point::new(cx, cy),
                11,
                if bottle.is_hidden() {
                    hud_progress_bg()
                } else {
                    detected_slot_border()
                },
                2,
                imgproc::LINE_AA,
                0,
            )?;

            if let Some(req) = bottle.hidden_requirement() {
                imgproc::circle(
                    frame_display,
                    Point::new(cx, cy),
                    5,
                    color_to_scalar(req),
                    imgproc::FILLED,
                    imgproc::LINE_AA,
                    0,
                )?;
                continue;
            }
        }

        let fills = bottle.get_fills();
        for layer_idx in 0..4 {
            let Some(water_sort_core::position::Pos(x, y)) =
                layout.get_sample_position(bottle_idx, layer_idx)
            else {
                continue;
            };

            let color = fills
                .get(3 - layer_idx)
                .copied()
                .map(color_to_scalar)
                .unwrap_or_else(detected_empty_fill);

            imgproc::circle(
                frame_display,
                Point::new(x, y),
                9,
                color,
                imgproc::FILLED,
                imgproc::LINE_AA,
                0,
            )?;
            imgproc::circle(
                frame_display,
                Point::new(x, y),
                9,
                detected_slot_border(),
                1,
                imgproc::LINE_AA,
                0,
            )?;
        }
    }

    Ok(())
}

#[cfg(feature = "solver-visualization")]
pub fn draw_solver_search_preview(
    frame_display: &mut Mat,
    bottles: &[Bottle],
    explored_states: usize,
    queue_len: usize,
    depth: usize,
    is_goal: bool,
) -> Result<()> {
    let bottle_count = bottles.len();
    if bottle_count == 0 {
        return Ok(());
    }

    let per_row = 7usize;
    let rows = bottle_count.div_ceil(per_row);

    let bottle_width = 24i32;
    let bottle_height = 76i32;
    let bottle_gap_x = 10i32;
    let bottle_gap_y = 16i32;
    let panel_padding = 12i32;

    let row_width = per_row as i32 * bottle_width + (per_row as i32 - 1) * bottle_gap_x;
    let panel_width = row_width + panel_padding * 2;
    let panel_height = 64 + rows as i32 * bottle_height + (rows as i32 - 1) * bottle_gap_y;

    let panel_x = 12;
    let panel_y = 12;

    imgproc::rectangle(
        frame_display,
        Rect::new(panel_x, panel_y, panel_width, panel_height),
        solver_preview_bg(),
        imgproc::FILLED,
        imgproc::LINE_8,
        0,
    )?;
    imgproc::rectangle(
        frame_display,
        Rect::new(panel_x, panel_y, panel_width, panel_height),
        solver_preview_border(),
        2,
        imgproc::LINE_8,
        0,
    )?;

    let title = if is_goal {
        "Solver Search (goal reached)"
    } else {
        "Solver Search"
    };

    imgproc::put_text(
        frame_display,
        title,
        Point::new(panel_x + 12, panel_y + 24),
        imgproc::FONT_HERSHEY_DUPLEX,
        0.58,
        hud_title(),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    imgproc::put_text(
        frame_display,
        &format!(
            "expanded {} | frontier {} | depth {}",
            explored_states, queue_len, depth
        ),
        Point::new(panel_x + 12, panel_y + 46),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.48,
        if is_goal {
            hud_progress_fill()
        } else {
            hud_muted()
        },
        1,
        imgproc::LINE_AA,
        false,
    )?;

    let grid_x = panel_x + panel_padding;
    let grid_y = panel_y + 58;

    for (index, bottle) in bottles.iter().enumerate() {
        let col = (index % per_row) as i32;
        let row = (index / per_row) as i32;

        let x = grid_x + col * (bottle_width + bottle_gap_x);
        let y = grid_y + row * (bottle_height + bottle_gap_y);

        let solved = bottle.is_solved();
        let border_color = if solved {
            hud_progress_fill()
        } else {
            solver_preview_border()
        };

        imgproc::rectangle(
            frame_display,
            Rect::new(x, y, bottle_width, bottle_height),
            border_color,
            2,
            imgproc::LINE_AA,
            0,
        )?;

        let fills = bottle.get_fills();
        let slot_height = 16i32;
        let slot_gap = 2i32;

        for layer in 0..4i32 {
            let slot_x = x + 3;
            let slot_y = y + bottle_height - 4 - (layer + 1) * slot_height - layer * slot_gap;
            let slot_w = bottle_width - 6;
            let slot_h = slot_height;

            let color = fills
                .get(layer as usize)
                .map(|fill| {
                    let pixel = fill.to_pixel_value();
                    Scalar::new(pixel[0] as f64, pixel[1] as f64, pixel[2] as f64, 0.0)
                })
                .unwrap_or_else(solver_empty_fill);

            imgproc::rectangle(
                frame_display,
                Rect::new(slot_x, slot_y, slot_w.max(1), slot_h.max(1)),
                color,
                imgproc::FILLED,
                imgproc::LINE_8,
                0,
            )?;
        }
    }

    Ok(())
}

#[cfg(feature = "solver-visualization")]
fn get_bottle_center_pixel(bottle_index: usize, layout: &BottleLayout) -> Option<(i32, i32)> {
    let pos = layout.get_click_position(bottle_index)?;
    Some((pos.0, pos.1))
}

#[cfg(feature = "solver-visualization")]
fn get_bottle_bounds(bottle_index: usize, layout: &BottleLayout) -> Option<(i32, i32, i32, i32)> {
    // Estimate bottle bounds based on positions
    // Get a few sample positions to estimate bottle dimensions
    let top_pos = layout.get_sample_position(bottle_index, 0)?;
    let bottom_pos = layout.get_sample_position(
        bottle_index,
        3.min(layout.positions[bottle_index].layer_offsets.len() - 1),
    )?;

    // Calculate approximate bottle width and height
    let bottle_width = 50i32; // Estimated bottle width
    let x0 = top_pos.0 - bottle_width / 2;
    let y0 = top_pos.1 - 30;
    let x1 = x0 + bottle_width;
    let y1 = bottom_pos.1 + 10;

    Some((x0, y0, x1, y1))
}

#[cfg(feature = "solver-visualization")]
fn draw_arrow(
    frame: &mut Mat,
    start: (i32, i32),
    end: (i32, i32),
    color: Scalar,
    thickness: i32,
) -> Result<()> {
    // Draw line from start to end
    imgproc::line(
        frame,
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        color,
        thickness,
        imgproc::LINE_AA,
        0,
    )?;

    // Draw arrowhead
    let dx = (end.0 - start.0) as f32;
    let dy = (end.1 - start.1) as f32;
    let distance = (dx * dx + dy * dy).sqrt();

    if distance > 0.0 {
        let angle = dy.atan2(dx);
        let arrow_size = 15.0;

        let tip1_x =
            (end.0 as f32 - arrow_size * angle.cos() + arrow_size * 0.5 * angle.sin()) as i32;
        let tip1_y =
            (end.1 as f32 - arrow_size * angle.sin() - arrow_size * 0.5 * angle.cos()) as i32;

        let tip2_x =
            (end.0 as f32 - arrow_size * angle.cos() - arrow_size * 0.5 * angle.sin()) as i32;
        let tip2_y =
            (end.1 as f32 - arrow_size * angle.sin() + arrow_size * 0.5 * angle.cos()) as i32;

        imgproc::line(
            frame,
            Point::new(end.0, end.1),
            Point::new(tip1_x, tip1_y),
            color,
            thickness,
            imgproc::LINE_AA,
            0,
        )?;
        imgproc::line(
            frame,
            Point::new(end.0, end.1),
            Point::new(tip2_x, tip2_y),
            color,
            thickness,
            imgproc::LINE_AA,
            0,
        )?;
    }

    Ok(())
}

#[cfg(feature = "solver-visualization")]
pub fn draw_solver_move_indicators(
    frame_display: &mut Mat,
    snapshot: &OverlaySnapshot<'_>,
) -> Result<()> {
    // Only draw if we have layout and are executing moves
    let Some(layout) = snapshot.solve_layout else {
        return Ok(());
    };
    if snapshot.solve_moves.is_empty() {
        return Ok(());
    }

    // Get the current move being executed
    if snapshot.solve_current_move_index >= snapshot.solve_moves.len() {
        return Ok(());
    }

    let current_move = snapshot.solve_moves[snapshot.solve_current_move_index];
    let source_idx = current_move.source_index();
    let dest_idx = current_move.destination_index();

    // Get pixel positions for bottles
    let source_pos = match get_bottle_center_pixel(source_idx, layout) {
        Some(pos) => pos,
        None => return Ok(()),
    };

    let dest_pos = match get_bottle_center_pixel(dest_idx, layout) {
        Some(pos) => pos,
        None => return Ok(()),
    };

    // Draw source bottle highlight
    if let Some((x0, y0, x1, y1)) = get_bottle_bounds(source_idx, layout) {
        let rect = Rect::new(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1));
        imgproc::rectangle(
            frame_display,
            rect,
            solver_source_highlight(),
            4,
            imgproc::LINE_AA,
            0,
        )?;
    }

    // Draw destination bottle highlight
    if let Some((x0, y0, x1, y1)) = get_bottle_bounds(dest_idx, layout) {
        let rect = Rect::new(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1));
        imgproc::rectangle(
            frame_display,
            rect,
            solver_destination_highlight(),
            4,
            imgproc::LINE_AA,
            0,
        )?;
    }

    // Draw arrow from source to destination
    draw_arrow(frame_display, source_pos, dest_pos, solver_arrow_color(), 3)?;

    Ok(())
}
