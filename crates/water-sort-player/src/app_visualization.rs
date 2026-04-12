use std::time::Duration;

use anyhow::Result;
use opencv::{
    core::{Mat, Point, Rect, Scalar},
    imgproc,
};

#[cfg(feature = "solver-visualization")]
use crate::bottles::BottleLayout;
use crate::solver::Move;

fn hud_background() -> Scalar {
    Scalar::new(22.0, 18.0, 14.0, 0.0)
}

fn hud_border() -> Scalar {
    Scalar::new(160.0, 146.0, 128.0, 0.0)
}

fn hud_title() -> Scalar {
    Scalar::new(235.0, 228.0, 215.0, 0.0)
}

fn hud_text() -> Scalar {
    Scalar::new(205.0, 196.0, 184.0, 0.0)
}

fn hud_muted() -> Scalar {
    Scalar::new(148.0, 138.0, 122.0, 0.0)
}

fn hud_accent() -> Scalar {
    Scalar::new(242.0, 172.0, 44.0, 0.0)
}

fn hud_progress_bg() -> Scalar {
    Scalar::new(82.0, 73.0, 62.0, 0.0)
}

fn hud_progress_fill() -> Scalar {
    Scalar::new(78.0, 197.0, 255.0, 0.0)
}

#[cfg(feature = "solver-visualization")]
fn solver_source_highlight() -> Scalar {
    // Bright red (BGR format)
    Scalar::new(0.0, 0.0, 255.0, 0.0)
}

#[cfg(feature = "solver-visualization")]
fn solver_destination_highlight() -> Scalar {
    // Bright cyan (BGR format)
    Scalar::new(255.0, 255.0, 0.0, 0.0)
}

#[cfg(feature = "solver-visualization")]
fn solver_arrow_color() -> Scalar {
    // Bright yellow (BGR format)
    Scalar::new(0.0, 255.0, 255.0, 0.0)
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
