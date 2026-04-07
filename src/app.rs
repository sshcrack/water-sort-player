use std::{
    collections::HashSet,
    io::{BufRead, BufReader, Write},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use minifb::{MouseButton, MouseMode, Window, WindowOptions};
use opencv::{
    core::{Mat, MatTraitConst, Vec3b},
    videoio::{self, VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};

use crate::{
    app_visualization::{OverlaySnapshot, draw_state_hud},
    bottles::{Bottle, BottleLayout, detect_bottles_with_layout},
    capture::{frame_to_window_buffer, save_frame_png},
    constants::{
        NEXT_LEVEL_BUTTON_POS, NO_THANK_YOU_REWARDS_POS, RETRY_BUTTON_POS, START_BUTTON_POS,
        VIRTUAL_CAM, is_color_within_tolerance,
    },
    scrcpy::{click_at_position, measure_window_to_mobile_scale, start_scrcpy},
    solver::{
        Move,
        discovery::{
            self, count_total_mystery_colors, find_best_discovery_moves,
            improve_best_revealed_state, reveal_mystery_colors_in_already_visited,
        },
        run_solver,
        visualization::draw_revealed_fill_markers,
    },
};

const START_WAIT: Duration = Duration::from_secs(10);
const NEXT_LEVEL_WAIT: Duration = Duration::from_secs(5);
const NO_THANK_YOU_REWARDS_WAIT: Duration = Duration::from_secs(10);
const MOVE_DELAY: Duration = Duration::from_millis(2500);
const DISCOVERY_MOVE_DELAY: Duration = Duration::from_millis(2500);

enum AppState {
    WaitingToPressStart {
        trigger_at: Instant,
    },
    ClickNextLevel {
        trigger_at: Instant,
    },
    CheckForRewards {
        trigger_at: Instant,
    },
    DetectAndPlan {
        trigger_at: Instant,
    },
    MysteryDiscoverColors {
        trigger_at: Instant,
        max_revealed_bottle_state: Vec<Bottle>,
        current_moves: Vec<Move>,
        already_visited_states: HashSet<Vec<Bottle>>,
    },
    MysteryExecuteDiscoverMove {
        trigger_at: Instant,
        moves_to_execute: Vec<Move>,
        max_revealed_bottle_state: Vec<Bottle>,
        current_moves: Vec<Move>,
        already_visited_states: HashSet<Vec<Bottle>>,
    },
    ExecuteFinalSolveMoves {
        next_move_at: Instant,
        planned_moves: Vec<Move>,
        performed_moves: usize,
    },
}

pub fn run(quick_mode: bool) -> Result<()> {
    if quick_mode {
        println!("Quick start mode enabled: skipping scrcpy startup and start-button automation.");
    }
    println!("Loading loopback video device...");
    load_loopback_device();

    let mut scrcpy = start_scrcpy(quick_mode)?;
    println!("scrcpy started successfully.");

    let child_stdout = scrcpy
        .take_stdout()
        .ok_or_else(|| anyhow!("failed to capture scrcpy stdout"))?;

    wait_for_video_stream(BufReader::new(child_stdout))?;

    thread::sleep(Duration::from_secs(2));

    let mut cam = VideoCapture::from_file(VIRTUAL_CAM, videoio::CAP_V4L2)?;

    let width = cam.get(videoio::CAP_PROP_FRAME_WIDTH)? as usize;
    let height = cam.get(videoio::CAP_PROP_FRAME_HEIGHT)? as usize;

    measure_window_to_mobile_scale(width, height);

    println!("Video stream dimensions: {}x{}", width, height);
    let mut window = Window::new("AutoPlayer", width, height, WindowOptions::default())?;

    let mut frame_raw = Mat::default();

    let mut app_state = if quick_mode {
        AppState::DetectAndPlan {
            trigger_at: Instant::now() + Duration::from_secs(1),
        }
    } else {
        AppState::WaitingToPressStart {
            trigger_at: Instant::now() + START_WAIT,
        }
    };
    let mut previous_right_click = false;
    let mut active_layout: Option<BottleLayout> = None;

    while window.is_open() {
        cam.read(&mut frame_raw)?;
        if frame_raw.empty() {
            continue;
        }

        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp)
            && window.get_mouse_down(MouseButton::Left)
        {
            println!("Clicked at: ({}, {})", x, y);
        }

        let right_click = window.get_mouse_down(MouseButton::Right);
        if right_click && !previous_right_click {
            let saved_path = save_frame_png(&frame_raw)?;
            println!("Saved quick-iteration frame to {}", saved_path.display());
        }
        previous_right_click = right_click;

        let now = Instant::now();
        let mut frame_display = frame_raw.try_clone()?;

        match &mut app_state {
            AppState::WaitingToPressStart { trigger_at } => {
                if now >= *trigger_at {
                    println!("Starting level...");
                    click_at_position(START_BUTTON_POS);
                    app_state = AppState::DetectAndPlan {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                    };
                }
            }
            AppState::ClickNextLevel { trigger_at } => {
                if now >= *trigger_at {
                    println!("Pressing next level button...");
                    // Check if it is the expected next level button color
                    let pixel = frame_raw
                        .at_2d::<Vec3b>(NEXT_LEVEL_BUTTON_POS.1, NEXT_LEVEL_BUTTON_POS.0)
                        .unwrap();

                    if is_color_within_tolerance(
                        pixel,
                        &crate::constants::NEXT_LEVEL_BUTTON_COLOR,
                        15,
                    ) {
                        click_at_position(NEXT_LEVEL_BUTTON_POS);
                    } else {
                        println!(
                            "Warning: Next level button color mismatch. Clicking configured position anyway..."
                        );
                        click_at_position(NEXT_LEVEL_BUTTON_POS);
                    }

                    app_state = AppState::DetectAndPlan {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                    };
                }
            }
            AppState::DetectAndPlan { trigger_at } => {
                if now >= *trigger_at {
                    println!("Detecting bottles for new level...");
                    let layout = match BottleLayout::detect_layout(&frame_raw) {
                        Ok(layout) => layout,
                        Err(error) => {
                            panic!(
                                "Layout detection failed with error: {:?}. Cannot proceed without layout.",
                                error
                            );
                        }
                    };
                    let bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, &layout);
                    if let Err(error) = bottles {
                        panic!(
                            "Could not detect bottles. Error: {:?}. Cannot proceed without bottle detection.",
                            error
                        );
                    } else {
                        active_layout = Some(layout.clone());

                        // Redraw window before running solver to show detected bottles
                        let buffer = frame_to_window_buffer(&frame_display)?;
                        window.update_with_buffer(&buffer, width, height)?;

                        std::thread::sleep(Duration::from_secs(1)); // Brief pause to show detected bottles before solver runs

                        let detected_bottles = bottles?;

                        // Check if there are any mystery colors
                        let mystery_count =
                            discovery::count_total_mystery_colors(&detected_bottles);
                        if mystery_count == 0 {
                            println!("No mystery colors detected, running solver directly...");
                            let solution = run_solver(&detected_bottles)
                                .expect("Failed to find a solution for the detected bottles");

                            app_state = AppState::ExecuteFinalSolveMoves {
                                planned_moves: solution,
                                performed_moves: 0,
                                next_move_at: Instant::now() + MOVE_DELAY,
                            };
                        } else {
                            println!(
                                "Detected {} mystery colors, starting discovery process...",
                                mystery_count
                            );

                            app_state = AppState::MysteryDiscoverColors {
                                trigger_at: now,
                                max_revealed_bottle_state: detected_bottles.clone(),
                                current_moves: vec![],
                                already_visited_states: HashSet::new(),
                            };
                        }
                    }
                }
            }
            AppState::MysteryDiscoverColors {
                trigger_at,
                max_revealed_bottle_state,
                current_moves,
                already_visited_states,
            } => {
                if now >= *trigger_at {
                    let layout = require_active_layout(&active_layout, "discovery move execution");

                    let current_bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, layout);

                    if let Err(error) = current_bottles {
                        panic!(
                            "Error detecting bottles during discovery process: {:?}",
                            error
                        );
                    }

                    let current_bottles = current_bottles.unwrap();

                    draw_revealed_fill_markers(
                        &mut frame_display,
                        layout,
                        &current_bottles,
                        max_revealed_bottle_state,
                    )?;

                    improve_best_revealed_state(max_revealed_bottle_state, &current_bottles);

                    let mystery_colors = count_total_mystery_colors(max_revealed_bottle_state);
                    println!("Total mystery colors still hidden: {}", mystery_colors);
                    if mystery_colors == 0 {
                        println!("All mystery colors revealed! Running solver...");
                        let solution = run_solver(max_revealed_bottle_state)
                            .expect("Failed to find a solution for the revealed bottle state");

                        println!("Resetting level for the solver...");
                        click_at_position(RETRY_BUTTON_POS);
                        app_state = AppState::ExecuteFinalSolveMoves {
                            planned_moves: solution,
                            performed_moves: 0,
                            next_move_at: Instant::now() + MOVE_DELAY,
                        };
                    } else {
                        // First update already visited state
                        reveal_mystery_colors_in_already_visited(
                            max_revealed_bottle_state,
                            already_visited_states,
                        );

                        // Find best move to reveal more colors
                        let best_move = find_best_discovery_moves(
                            current_moves,
                            max_revealed_bottle_state,
                            already_visited_states,
                        );

                        match best_move {
                            discovery::DiscoverResult::MoveToDiscover(best_moves) => {
                                #[cfg(feature = "discovery-debugging")]
                                {
                                    println!("Best discovery move sequence found: {:?}", best_moves);
                                }
                                app_state = AppState::MysteryExecuteDiscoverMove {
                                    moves_to_execute: best_moves,
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    current_moves: current_moves.clone(),
                                    already_visited_states: already_visited_states.clone(),
                                    trigger_at: now,
                                };
                            }
                            discovery::DiscoverResult::NoMove => {
                                println!(
                                    "No discovery move found that reveals new colors. Retrying level..."
                                );

                                click_at_position(RETRY_BUTTON_POS);

                                app_state = AppState::MysteryDiscoverColors {
                                    trigger_at: Instant::now() + DISCOVERY_MOVE_DELAY,
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    current_moves: vec![],
                                    already_visited_states: already_visited_states.clone(),
                                };
                            }
                            discovery::DiscoverResult::AlreadySolved => {
                                println!(
                                    "While discovering, the puzzle has been solved. Proceeding to next level..."
                                );
                                app_state = AppState::CheckForRewards {
                                    trigger_at: now + NEXT_LEVEL_WAIT,
                                };
                            }
                        }
                    }
                }
            }
            AppState::MysteryExecuteDiscoverMove {
                trigger_at,
                moves_to_execute,
                max_revealed_bottle_state,
                current_moves,
                already_visited_states,
            } => {
                if now >= *trigger_at {
                    let layout = require_active_layout(&active_layout, "discovery move execution");

                    let current_bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, layout)?;

                    draw_revealed_fill_markers(
                        &mut frame_display,
                        layout,
                        &current_bottles,
                        max_revealed_bottle_state,
                    )?;

                    if moves_to_execute.is_empty() {
                        app_state = AppState::MysteryDiscoverColors {
                            trigger_at: now,
                            max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                            current_moves: current_moves.clone(),
                            already_visited_states: already_visited_states.clone(),
                        };
                    } else {
                        let next_move = moves_to_execute[0];

                        println!("Performing discovery move: {:?}.", next_move);
                        #[cfg(feature = "discovery-debugging")]
                        {
                            println!("Press enter to perform the next move...");
                            std::io::stdin().read_line(&mut String::new()).unwrap();
                        }
                        next_move.perform_move_on_device(layout);

                        // Remove the executed move from the list
                        moves_to_execute.remove(0);
                        current_moves.push(next_move);

                        // Schedule the next move or go back to discovery state after a delay
                        *trigger_at = Instant::now() + DISCOVERY_MOVE_DELAY;
                    }
                }
            }
            AppState::ExecuteFinalSolveMoves {
                planned_moves,
                performed_moves,
                next_move_at,
            } => {
                if let Some(next) = planned_moves.get(*performed_moves).copied() {
                    if now >= *next_move_at {
                        println!("Performing move: {:?}.", next);
                        let layout = require_active_layout(&active_layout, "solve move execution");
                        next.perform_move_on_device(layout);
                        *performed_moves += 1;
                        *next_move_at = now + MOVE_DELAY;
                    }
                } else {
                    app_state = AppState::CheckForRewards {
                        trigger_at: now + NO_THANK_YOU_REWARDS_WAIT,
                    };
                }
            }
            AppState::CheckForRewards { trigger_at } => {
                if now >= *trigger_at {
                    let pixel = frame_raw
                        .at_2d::<Vec3b>(NO_THANK_YOU_REWARDS_POS.1, NO_THANK_YOU_REWARDS_POS.0)?;

                    if is_color_within_tolerance(
                        pixel,
                        &crate::constants::NO_THANK_YOU_REWARDS_COLOR,
                        10,
                    ) {
                        println!("Reward screen detected, clicking 'No, thank you'...");
                        click_at_position(NO_THANK_YOU_REWARDS_POS);
                    } else {
                        println!("No reward screen detected, proceeding to next level...");
                    }

                    app_state = AppState::ClickNextLevel {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                    };
                }
            }
        }

        let overlay_snapshot = build_overlay_snapshot(&app_state, now);

        draw_state_hud(&mut frame_display, width, &overlay_snapshot)?;

        let buffer = frame_to_window_buffer(&frame_display)?;
        window.update_with_buffer(&buffer, width, height)?;
    }

    Ok(())
}

fn load_loopback_device() {
    Command::new("sudo")
        .args(["modprobe", "v4l2loopback", "video_nr=10"])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn wait_for_video_stream<R: BufRead>(mut reader: R) -> Result<()> {
    let mut line = String::new();
    print!("Waiting for scrcpy to initialize video stream...");

    loop {
        let bytes_read = reader.read_line(&mut line)?;
        print!(".");
        std::io::stdout().flush().unwrap();

        if bytes_read == 0 {
            return Err(anyhow!("scrcpy process ended unexpectedly"));
        }

        if line.contains("v4l2 sink started to device:") {
            println!("\nscrcpy is ready, starting video capture...");
            break;
        }

        line.clear();
    }

    Ok(())
}

fn require_active_layout<'a>(
    active_layout: &'a Option<BottleLayout>,
    context: &str,
) -> &'a BottleLayout {
    active_layout
        .as_ref()
        .unwrap_or_else(|| panic!("No active layout available for {}.", context))
}

fn remaining_until(trigger_at: Instant, now: Instant) -> Option<Duration> {
    if trigger_at > now {
        Some(trigger_at.duration_since(now))
    } else {
        None
    }
}

fn build_overlay_snapshot<'a>(app_state: &'a AppState, now: Instant) -> OverlaySnapshot<'a> {
    match app_state {
        AppState::WaitingToPressStart { trigger_at } => OverlaySnapshot {
            phase: "WaitingToPressStart".to_string(),
            detail: "Preparing initial level start tap".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
        },
        AppState::ClickNextLevel { trigger_at } => OverlaySnapshot {
            phase: "ClickNextLevel".to_string(),
            detail: "Waiting to advance to the next level".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
        },
        AppState::CheckForRewards { trigger_at } => OverlaySnapshot {
            phase: "CheckForRewards".to_string(),
            detail: "Looking for reward popup".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
        },
        AppState::DetectAndPlan { trigger_at } => OverlaySnapshot {
            phase: "DetectAndPlan".to_string(),
            detail: "Detecting bottle layout and planning".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
        },
        AppState::MysteryDiscoverColors {
            trigger_at,
            max_revealed_bottle_state,
            current_moves,
            ..
        } => OverlaySnapshot {
            phase: "MysteryDiscoverColors".to_string(),
            detail: "Scanning bottles to reveal mystery colors".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: Some(count_total_mystery_colors(max_revealed_bottle_state)),
            discovery_total_slots: Some(max_revealed_bottle_state.len() * 4),
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(0),
            solve_moves: &[],
            solve_performed_moves: 0,
        },
        AppState::MysteryExecuteDiscoverMove {
            trigger_at,
            moves_to_execute,
            max_revealed_bottle_state,
            current_moves,
            ..
        } => OverlaySnapshot {
            phase: "MysteryExecuteDiscoverMove".to_string(),
            detail: "Executing discovery sequence".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: Some(count_total_mystery_colors(max_revealed_bottle_state)),
            discovery_total_slots: Some(max_revealed_bottle_state.len() * 4),
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(moves_to_execute.len()),
            solve_moves: &[],
            solve_performed_moves: 0,
        },
        AppState::ExecuteFinalSolveMoves {
            next_move_at,
            planned_moves,
            performed_moves,
        } => OverlaySnapshot {
            phase: "ExecuteFinalSolveMoves".to_string(),
            detail: format!(
                "Running solver move {} of {}",
                performed_moves.saturating_add(1).min(planned_moves.len()),
                planned_moves.len()
            ),
            until_ready: remaining_until(*next_move_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: planned_moves.as_slice(),
            solve_performed_moves: *performed_moves,
        },
    }
}
