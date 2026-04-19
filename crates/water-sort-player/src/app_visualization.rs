use std::time::Duration;

use anyhow::Result;
use opencv::{
    core::{Mat, Point, Rect, Scalar},
    imgproc,
};
use water_sort_core::constants::scalar_from_hex;

use crate::bottles::Bottle;
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
    pub motion_status: Option<String>,
    pub discovery_hidden: Option<usize>,
    pub discovery_total_slots: Option<usize>,
    pub discovery_depth: Option<usize>,
    pub discovery_queue: Option<usize>,
    pub solve_moves: &'a [Move],
    pub solve_performed_moves: usize,
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
        .map(|e| format!("Next action: {}", format_duration(e)))
        .or_else(|| snapshot.motion_status.clone())
        .unwrap_or_else(|| "Next action: ready".to_string());
    imgproc::put_text(
        frame_display,
        &timing,
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

pub fn draw_detected_bottles_overlay(frame_display: &mut Mat, bottles: &[Bottle]) -> Result<()> {
    let slot_size = 12;
    for bottle in bottles {
        let Some(click_pos) = *bottle.click_position() else {
            continue;
        };

        // Layer preview spacing mirrors runtime bottle fill spacing and stays dynamic per bottle center.
        let layer_spacing = 28;

        for layer_index in 0..4usize {
            let offset_y = (layer_index as i32 * layer_spacing) - (layer_spacing * 3 / 2);
            let sample_pos = crate::position::Pos(click_pos.0, click_pos.1 + offset_y);

            let color = bottle
                .get_fills()
                .get(3usize.saturating_sub(layer_index))
                .copied()
                .map(color_to_scalar)
                .unwrap_or_else(detected_empty_fill);

            imgproc::rectangle(
                frame_display,
                Rect::new(
                    sample_pos.0 - slot_size / 2,
                    sample_pos.1 - slot_size / 2,
                    slot_size,
                    slot_size,
                ),
                color,
                imgproc::FILLED,
                imgproc::LINE_AA,
                0,
            )?;

            imgproc::rectangle(
                frame_display,
                Rect::new(
                    sample_pos.0 - slot_size / 2,
                    sample_pos.1 - slot_size / 2,
                    slot_size,
                    slot_size,
                ),
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
    if snapshot.solve_moves.is_empty() {
        return Ok(());
    }

    if snapshot.solve_current_move_index >= snapshot.solve_moves.len() {
        return Ok(());
    }

    let current_move = &snapshot.solve_moves[snapshot.solve_current_move_index];
    let Some(source_pos) = current_move.source_clickable_pos() else {
        return Ok(());
    };
    let Some(dest_pos) = current_move.destination_clickable_pos() else {
        return Ok(());
    };

    let source = (source_pos.0, source_pos.1);
    let destination = (dest_pos.0, dest_pos.1);

    imgproc::circle(
        frame_display,
        Point::new(source.0, source.1),
        20,
        solver_source_highlight(),
        3,
        imgproc::LINE_AA,
        0,
    )?;

    imgproc::circle(
        frame_display,
        Point::new(destination.0, destination.1),
        20,
        solver_destination_highlight(),
        3,
        imgproc::LINE_AA,
        0,
    )?;

    draw_arrow(frame_display, source, destination, solver_arrow_color(), 3)?;

    imgproc::put_text(
        frame_display,
        &format!(
            "{} -> {}",
            current_move.source_index(),
            current_move.destination_index()
        ),
        Point::new(source.0 + 14, source.1 - 14),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.58,
        solver_arrow_color(),
        2,
        imgproc::LINE_AA,
        false,
    )?;

    Ok(())
}
