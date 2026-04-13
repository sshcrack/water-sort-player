use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use clap::Parser;
use log::{debug, error, info, trace, warn};
use minifb::{Key, MouseButton, Window, WindowOptions};
use opencv::{
    core::{Mat, MatTraitConst, Point, Rect, Scalar},
    imgproc,
};
use serde_json::Value;
use water_sort_capture::frame_to_window_buffer;
use water_sort_core::{
    bottles::{Bottle, test_utils::TestUtils},
    constants::BottleColor,
};
use water_sort_solver::{Move, SolverProgressSnapshot, run_solver_with_progress};

#[derive(Debug, Clone)]
struct DiscoveryLevelEntry {
    id: u64,
    layout_name: String,
    expected_bottles: String,
    resolved_bottles: Option<String>,
    mystery_count_at_start: usize,
}

#[derive(Debug, Parser)]
#[command(
    name = "water-sort-discovery-solver",
    about = "Run the water sort solver on a discovery level with visualization"
)]
struct CliArgs {
    #[arg(value_name = "LEVEL_ID")]
    level_id: u64,

    #[arg(long, action = clap::ArgAction::SetTrue, default_value_t = true, help = "Pause on each rendered step until the left mouse button is clicked")]
    debug: bool,

    #[arg(long = "no-debug", action = clap::ArgAction::SetTrue, help = "Disable click-to-advance mode and use timed delays")]
    no_debug: bool,

    #[arg(
        long,
        default_value_t = 900,
        help = "Delay between rendered solver snapshots when debug stepping is disabled"
    )]
    snapshot_delay_ms: u64,

    #[arg(
        long,
        default_value_t = 350,
        help = "Delay between replayed solution moves when debug stepping is disabled"
    )]
    replay_delay_ms: u64,
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../captures/discovery_levels.json")
}

fn read_manifest() -> Result<Vec<DiscoveryLevelEntry>> {
    let manifest_text = fs::read_to_string(manifest_path())?;
    let parsed = serde_json::from_str::<Value>(&manifest_text)?;
    let Some(levels) = parsed.get("levels").and_then(|value| value.as_array()) else {
        return Ok(Vec::new());
    };

    let mut entries = Vec::with_capacity(levels.len());
    for level in levels {
        let id = level
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| anyhow!("discovery manifest entry is missing an id"))?;
        let layout_name = level
            .get("layout_name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("discovery manifest entry {id} is missing a layout_name"))?
            .to_string();
        let expected_bottles = level
            .get("expected_bottles")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("discovery manifest entry {id} is missing expected_bottles"))?
            .to_string();
        let resolved_bottles = level
            .get("resolved_bottles")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mystery_count_at_start = level
            .get("mystery_count_at_start")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                anyhow!("discovery manifest entry {id} is missing mystery_count_at_start")
            })? as usize;

        entries.push(DiscoveryLevelEntry {
            id,
            layout_name,
            expected_bottles,
            resolved_bottles,
            mystery_count_at_start,
        });
    }

    Ok(entries)
}

fn find_level(level_id: u64) -> Result<DiscoveryLevelEntry> {
    let entries = read_manifest()?;
    entries
        .into_iter()
        .find(|entry| entry.id == level_id)
        .ok_or_else(|| {
            let available = read_manifest()
                .map(|entries| {
                    entries
                        .into_iter()
                        .map(|entry| entry.id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_else(|_| "unknown".to_string());
            anyhow!("level id {level_id} was not found. Available ids: {available}")
        })
}

fn bottles_as_string(bottles: &[Bottle]) -> String {
    bottles
        .iter()
        .map(|bottle| bottle.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn log_bottles(prefix: &str, bottles: &[Bottle]) {
    trace!("{prefix}");
    for (index, bottle) in bottles.iter().enumerate() {
        trace!("  bottle[{index}]: {}", bottle);
    }
}

fn color_to_scalar(color: BottleColor) -> Scalar {
    let pixel = color.to_pixel_value();
    Scalar::new(pixel[0] as f64, pixel[1] as f64, pixel[2] as f64, 0.0)
}

fn render_background(frame: &mut Mat) -> Result<()> {
    imgproc::rectangle(
        frame,
        Rect::new(0, 0, frame.cols(), frame.rows()),
        Scalar::new(17.0, 17.0, 17.0, 0.0),
        imgproc::FILLED,
        imgproc::LINE_8,
        0,
    )?;
    Ok(())
}

fn draw_bottle(
    frame: &mut Mat,
    bottle: &Bottle,
    x: i32,
    y: i32,
    bottle_width: i32,
    bottle_height: i32,
) -> Result<()> {
    let border_color = if bottle.is_solved() {
        Scalar::new(80.0, 208.0, 80.0, 0.0)
    } else {
        Scalar::new(138.0, 138.0, 138.0, 0.0)
    };

    imgproc::rectangle(
        frame,
        Rect::new(x, y, bottle_width, bottle_height),
        border_color,
        2,
        imgproc::LINE_AA,
        0,
    )?;

    if let Some(requirement) = bottle.hidden_requirement() {
        imgproc::put_text(
            frame,
            &format!("!{}", requirement.to_char()),
            Point::new(x + 8, y + 20),
            imgproc::FONT_HERSHEY_DUPLEX,
            0.52,
            color_to_scalar(requirement),
            1,
            imgproc::LINE_AA,
            false,
        )?;
    }

    let fills = bottle.get_fills();
    let slot_height = 26;
    let slot_gap = 4;

    for slot in 0..4i32 {
        let slot_x = x + 6;
        let slot_y = y + bottle_height - 8 - (slot + 1) * slot_height - slot * slot_gap;
        let slot_w = bottle_width - 12;
        let slot_color = fills
            .get(slot as usize)
            .copied()
            .map(color_to_scalar)
            .unwrap_or_else(|| Scalar::new(52.0, 52.0, 52.0, 0.0));

        imgproc::rectangle(
            frame,
            Rect::new(slot_x, slot_y, slot_w, slot_height),
            slot_color,
            imgproc::FILLED,
            imgproc::LINE_8,
            0,
        )?;
        imgproc::rectangle(
            frame,
            Rect::new(slot_x, slot_y, slot_w, slot_height),
            Scalar::new(32.0, 32.0, 32.0, 0.0),
            1,
            imgproc::LINE_AA,
            0,
        )?;
    }

    Ok(())
}

fn draw_title(frame: &mut Mat, width: i32, level: &DiscoveryLevelEntry) -> Result<()> {
    imgproc::rectangle(
        frame,
        Rect::new(12, 12, (width - 24).max(0), 46),
        Scalar::new(27.0, 27.0, 27.0, 0.0),
        imgproc::FILLED,
        imgproc::LINE_8,
        0,
    )?;
    imgproc::rectangle(
        frame,
        Rect::new(12, 12, (width - 24).max(0), 46),
        Scalar::new(44.0, 196.0, 255.0, 0.0),
        2,
        imgproc::LINE_AA,
        0,
    )?;

    imgproc::put_text(
        frame,
        &format!("Discovery Solver - level {}", level.id),
        Point::new(24, 32),
        imgproc::FONT_HERSHEY_DUPLEX,
        0.62,
        Scalar::new(235.0, 228.0, 215.0, 0.0),
        1,
        imgproc::LINE_AA,
        false,
    )?;
    imgproc::put_text(
        frame,
        &format!(
            "layout {} | expected {}",
            level.layout_name, level.expected_bottles
        ),
        Point::new(24, 53),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.5,
        Scalar::new(184.0, 196.0, 205.0, 0.0),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    Ok(())
}

fn draw_state_info(frame: &mut Mat, bottles: &[Bottle], line_y: i32, label: &str) -> Result<()> {
    imgproc::put_text(
        frame,
        label,
        Point::new(24, line_y),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.5,
        Scalar::new(122.0, 138.0, 148.0, 0.0),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    let preview = bottles
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");

    imgproc::put_text(
        frame,
        &preview,
        Point::new(24, line_y + 22),
        imgproc::FONT_HERSHEY_SIMPLEX,
        0.44,
        Scalar::new(205.0, 196.0, 184.0, 0.0),
        1,
        imgproc::LINE_AA,
        false,
    )?;

    Ok(())
}

fn wait_for_left_click(window: &mut Window) -> usize {
    let mut was_left_down = window.get_mouse_down(MouseButton::Left);
    let mut was_right_down = window.get_mouse_down(MouseButton::Right);

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update();

        let is_left_down = window.get_mouse_down(MouseButton::Left);
        if is_left_down && !was_left_down {
            while window.is_open() && window.get_mouse_down(MouseButton::Left) {
                window.update();
                std::thread::sleep(Duration::from_millis(16));
            }
            break;
        }

        let is_right_down = window.get_mouse_down(MouseButton::Right);
        if is_right_down && !was_right_down {
            while window.is_open() && window.get_mouse_down(MouseButton::Right) {
                window.update();
                std::thread::sleep(Duration::from_millis(16));
            }

            return 10;
        }

        was_left_down = is_left_down;
        was_right_down = is_right_down;
        std::thread::sleep(Duration::from_millis(16));
    }

    0
}

fn render_state(frame: &mut Mat, bottles: &[Bottle], level: &DiscoveryLevelEntry) -> Result<()> {
    render_background(frame)?;
    draw_title(frame, frame.cols(), level)?;

    let columns = bottles.len().clamp(1, 7);
    let bottle_width = 54i32;
    let bottle_height = 150i32;
    let gap_x = 14i32;
    let gap_y = 18i32;
    let padding_x = 18i32;
    let grid_top = 86i32;

    draw_state_info(frame, bottles, 72, "bottles")?;

    for (index, bottle) in bottles.iter().enumerate() {
        let column = (index % columns) as i32;
        let row = (index / columns) as i32;

        let x = padding_x + column * (bottle_width + gap_x);
        let y = grid_top + row * (bottle_height + gap_y);

        draw_bottle(frame, bottle, x, y, bottle_width, bottle_height)?;
    }

    Ok(())
}

fn preview_size(bottle_count: usize) -> (usize, usize) {
    let columns = bottle_count.clamp(1, 7);
    let rows = bottle_count.div_ceil(columns);

    let bottle_width = 54usize;
    let bottle_height = 150usize;
    let gap_x = 14usize;
    let gap_y = 18usize;
    let padding_x = 18usize;
    let padding_y = 24usize;

    let width = padding_x * 2 + columns * bottle_width + columns.saturating_sub(1) * gap_x;
    let height = 68 + padding_y * 2 + rows * bottle_height + rows.saturating_sub(1) * gap_y;
    (width.max(360), height.max(280))
}

fn render_and_update(
    window: &mut Window,
    frame: &mut Mat,
    bottles: &[Bottle],
    level: &DiscoveryLevelEntry,
) -> Result<()> {
    render_state(frame, bottles, level)?;
    let buffer = frame_to_window_buffer(frame)?;
    if let Err(error) =
        window.update_with_buffer(&buffer, frame.cols() as usize, frame.rows() as usize)
    {
        warn!("failed to update visualization window: {error:?}");
    }
    Ok(())
}

fn render_step(
    window: &mut Window,
    frame: &mut Mat,
    bottles: &[Bottle],
    level: &DiscoveryLevelEntry,
    debug_mode: bool,
    fallback_delay: Duration,
    pending_debug_skip_snapshots: &mut usize,
) -> Result<()> {
    render_and_update(window, frame, bottles, level)?;

    if debug_mode {
        if *pending_debug_skip_snapshots > 0 {
            *pending_debug_skip_snapshots -= 1;
        } else {
            *pending_debug_skip_snapshots = wait_for_left_click(window);
        }
    } else {
        std::thread::sleep(fallback_delay);
    }

    Ok(())
}

fn replay_solution(
    window: &mut Window,
    frame: &mut Mat,
    initial_bottles: &[Bottle],
    level: &DiscoveryLevelEntry,
    moves: &[Move],
    debug_mode: bool,
    replay_delay: Duration,
) -> Result<()> {
    let mut state = initial_bottles.to_vec();
    let mut pending_debug_skip_snapshots = 0usize;
    trace!("Replaying {} solution moves", moves.len());

    for (index, mv) in moves.iter().enumerate() {
        debug!(
            "replay move {}: {} -> {}",
            index + 1,
            mv.source_index(),
            mv.destination_index()
        );
        trace!("before move {}: {}", index + 1, bottles_as_string(&state));
        mv.perform_move_on_bottles(&mut state);
        trace!("after move {}: {}", index + 1, bottles_as_string(&state));
        render_step(
            window,
            frame,
            &state,
            level,
            debug_mode,
            replay_delay,
            &mut pending_debug_skip_snapshots,
        )?;
    }

    Ok(())
}

fn run() -> Result<()> {
    let args = CliArgs::parse();
    let debug_mode = args.debug && !args.no_debug;
    let level = find_level(args.level_id)?;

    info!(
        "Loaded discovery level {} from {} ({}, mystery_count_at_start={})",
        level.id, level.layout_name, level.expected_bottles, level.mystery_count_at_start
    );

    let expected_bottles = TestUtils::parse_bottles_sequence(&level.expected_bottles);
    let resolved_bottles = level
        .resolved_bottles
        .as_deref()
        .map(TestUtils::parse_bottles_sequence)
        .unwrap_or_else(|| expected_bottles.clone());

    debug!("Expected bottles: {}", bottles_as_string(&expected_bottles));
    debug!("Resolved bottles: {}", bottles_as_string(&resolved_bottles));
    log_bottles("Expected bottle breakdown", &expected_bottles);
    log_bottles("Resolved bottle breakdown", &resolved_bottles);

    let (width, height) = preview_size(expected_bottles.len());
    let mut window = Window::new(
        &format!("Water Sort Level {}", level.id),
        width,
        height,
        WindowOptions {
            resize: false,
            scale: minifb::Scale::X1,
            ..WindowOptions::default()
        },
    )?;
    let mut frame = Mat::new_rows_cols_with_default(
        height as i32,
        width as i32,
        opencv::core::CV_8UC3,
        Scalar::new(17.0, 17.0, 17.0, 0.0),
    )?;
    let mut pending_debug_skip_snapshots = 0usize;

    render_and_update(&mut window, &mut frame, &expected_bottles, &level)?;
    if debug_mode {
        pending_debug_skip_snapshots = wait_for_left_click(&mut window);
    } else {
        std::thread::sleep(Duration::from_millis(args.snapshot_delay_ms));
    }

    let mut last_render = Instant::now();
    let snapshot_delay = Duration::from_millis(args.snapshot_delay_ms);
    let mut on_progress = |snapshot: SolverProgressSnapshot<'_>| {
        if !window.is_open() {
            return;
        }

        trace!(
            "solver snapshot: explored={} queue={} depth={} goal={}",
            snapshot.explored_states, snapshot.queue_len, snapshot.depth, snapshot.is_goal
        );
        debug!("solver state: {}", bottles_as_string(snapshot.state));
        log_bottles("solver bottle breakdown", snapshot.state);

        if !debug_mode && last_render.elapsed() < snapshot_delay && !snapshot.is_goal {
            return;
        }

        last_render = Instant::now();
        if let Err(error) = render_step(
            &mut window,
            &mut frame,
            snapshot.state,
            &level,
            debug_mode,
            snapshot_delay,
            &mut pending_debug_skip_snapshots,
        ) {
            warn!("failed to render solver snapshot: {error:#}");
        }
    };

    let mut solution =
        run_solver_with_progress(&resolved_bottles, &expected_bottles, &mut on_progress);

    if solution.is_none() {
        warn!(
            "Solver could not find a path with expected-state mystery constraints. Retrying with resolved state as initial..."
        );
        solution = run_solver_with_progress(&resolved_bottles, &resolved_bottles, &mut on_progress);
    }

    let solution = solution.ok_or_else(|| anyhow!("failed to solve level {}", level.id))?;

    info!("Solution found with {} moves", solution.len());
    for (index, mv) in solution.iter().enumerate() {
        debug!(
            "solution move {}: {} -> {}",
            index + 1,
            mv.source_index(),
            mv.destination_index()
        );
    }

    replay_solution(
        &mut window,
        &mut frame,
        &expected_bottles,
        &level,
        &solution,
        debug_mode,
        Duration::from_millis(args.replay_delay_ms),
    )?;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        window.update();
        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("trace"))
        .format_timestamp_millis()
        .init();

    if let Err(error) = run() {
        error!("Error: {error:#}");
        std::process::exit(1);
    }
}
