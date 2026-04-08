use std::time::Duration;

use anyhow::Result;
use opencv::{
    core::{Mat, Point, Rect, Scalar},
    imgproc,
};

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

    Ok(())
}
