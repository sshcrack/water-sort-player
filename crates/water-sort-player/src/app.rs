use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use minifb::{MouseButton, MouseMode, Window, WindowOptions};
use opencv::core::{Mat, MatTraitConst, Vec3b};
use water_sort_core::constants::{
    NEXT_LEVEL_BUTTON_COLOR, NEXT_LEVEL_BUTTON_POSITIONS, NO_THANK_YOU_POSITIONS,
    NO_THANK_YOU_REWARDS_COLOR, color_distance_sq,
};
use water_sort_device::{CaptureDeviceBackend, construct_capture_backend};

use crate::{
    app_visualization::{OverlaySnapshot, draw_detected_bottles_overlay, draw_state_hud},
    bottles::{Bottle, BottleLayout, detect_bottles_with_layout},
    capture::{DiscoveryCaptureContext, frame_to_window_buffer, save_frame_png},
    constants::{RETRY_BUTTON_POS, START_BUTTON_POS},
    solver::{
        Move,
        discovery::{
            self, collect_hidden_requirements, count_hidden_bottles, count_total_mystery_colors,
            find_best_discovery_moves, find_best_hidden_unlock_moves, improve_best_revealed_state,
            improve_current_bottles_with_revealed_state,
        },
        visualization::draw_revealed_fill_markers,
    },
};

#[cfg(feature = "solver-visualization")]
use crate::app_visualization::draw_solver_search_preview;
#[cfg(not(feature = "solver-visualization"))]
use crate::solver::run_solver;

#[cfg(feature = "collect-test-data")]
use crate::capture::start_discovery_capture;

const START_WAIT: Duration = Duration::from_secs(10);
const NEXT_LEVEL_WAIT: Duration = Duration::from_secs(6);
const NO_THANK_YOU_REWARDS_WAIT: Duration = Duration::from_secs(15);
const BOTTLE_DETECTION_RETRY_DELAY: Duration = Duration::from_secs(1);
const BOTTLE_DETECTION_RETRIES: u8 = 3;
const POST_DETECTION_WAIT: Duration = Duration::from_secs(1);
const MOVE_DELAY: Duration = Duration::from_millis(3000);
const DISCOVERY_MOVE_DELAY: Duration = Duration::from_millis(3000);
const HIDDEN_REVEAL_DETECTION_DELAY: Duration = Duration::from_millis(5500);
#[cfg(feature = "solver-visualization")]
const SOLVER_VISUALIZATION_UPDATE_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(feature = "solver-visualization")]
const SOLVER_VISUALIZATION_FRAME_DELAY: Duration = Duration::from_millis(20);

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
        retries_remaining: u8,
    },
    AwaitPostDetectionPlan {
        trigger_at: Instant,
        detected_bottles: Vec<Bottle>,
    },
    HiddenDiscoverBottles {
        trigger_at: Instant,
        current_moves: Vec<Move>,
    },
    HiddenExecuteDiscoverMove {
        trigger_at: Instant,
        moves_to_execute: Vec<Move>,
        current_moves: Vec<Move>,
    },
    MysteryDiscoverColors {
        trigger_at: Instant,
        initial_state: Vec<Bottle>,
        max_revealed_bottle_state: Vec<Bottle>,
        current_moves: Vec<Move>,
    },
    MysteryExecuteDiscoverMove {
        trigger_at: Instant,
        moves_to_execute: Vec<Move>,
        initial_state: Vec<Bottle>,
        max_revealed_bottle_state: Vec<Bottle>,
        current_moves: Vec<Move>,
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
    #[cfg(feature = "collect-test-data")]
    println!("Test data collection enabled (feature: collect-test-data).");

    let mut capture = construct_capture_backend();
    let (width, height) = capture.start_capture(quick_mode)?;
    println!("Creating window...");
    let mut window = Window::new("AutoPlayer", width, height, WindowOptions::default())?;
    let mut frame_raw: Mat;

    let mut app_state = if quick_mode {
        AppState::DetectAndPlan {
            trigger_at: Instant::now() + Duration::from_secs(1),
            retries_remaining: BOTTLE_DETECTION_RETRIES,
        }
    } else {
        AppState::WaitingToPressStart {
            trigger_at: Instant::now() + START_WAIT,
        }
    };
    let mut previous_right_click = false;
    let mut active_layout: Option<BottleLayout> = None;
    let mut latest_detected_bottles: Option<Vec<Bottle>> = None;
    let mut discovery_capture: Option<DiscoveryCaptureContext> = None;

    let mut first_frame_read = true;
    while window.is_open() {
        if first_frame_read {
            println!("Reading first frame...");
            first_frame_read = false;
        }
        match capture.capture_frame() {
            Ok(frame) => frame_raw = frame,
            Err(error) => {
                println!("Skipping frame read error: {:?}", error);
                continue;
            }
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
                    capture.click_at_position(START_BUTTON_POS)?;
                    app_state = AppState::DetectAndPlan {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                        retries_remaining: BOTTLE_DETECTION_RETRIES,
                    };
                }
            }
            AppState::ClickNextLevel { trigger_at } => {
                if now >= *trigger_at {
                    let button_pos = NEXT_LEVEL_BUTTON_POSITIONS
                        .iter()
                        .find(|pos| {
                            let pixel = frame_raw.at_2d::<Vec3b>(pos.1, pos.0).unwrap();
                            println!("Checking for next level button at position {:?} with pixel value {:?}...", pos, pixel);
                            println!("Color distance to expected next level button color: {}", color_distance_sq(pixel, &NEXT_LEVEL_BUTTON_COLOR));
                            println!("Hex value of pixel: #{:02x}{:02x}{:02x}", pixel[2], pixel[1], pixel[0]);
                            color_distance_sq(pixel, &NEXT_LEVEL_BUTTON_COLOR)
                                <= 50 * 50
                        })
                        .copied()
                        .unwrap_or(NEXT_LEVEL_BUTTON_POSITIONS[0]);
                    println!("Pressing next level button at {button_pos:?}...");

                    capture.click_at_position(button_pos)?;
                    app_state = AppState::DetectAndPlan {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                        retries_remaining: BOTTLE_DETECTION_RETRIES,
                    };
                }
            }
            AppState::DetectAndPlan {
                trigger_at,
                retries_remaining,
            } => {
                if now >= *trigger_at {
                    println!("Detecting bottles for new level...");
                    let layout = match BottleLayout::detect_layout(&frame_raw) {
                        Ok(layout) => layout,
                        Err(error) => {
                            return Err(anyhow!(
                                "Layout detection failed with error: {:?}. Cannot proceed without layout.",
                                error
                            ));
                        }
                    };
                    let bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, &layout);
                    match bottles {
                        Ok(detected_bottles) => {
                            active_layout = Some(layout.clone());
                            latest_detected_bottles = Some(detected_bottles.clone());
                            app_state = AppState::AwaitPostDetectionPlan {
                                trigger_at: now + POST_DETECTION_WAIT,
                                detected_bottles,
                            };
                        }
                        Err(error) => {
                            if *retries_remaining == 0 {
                                println!("No retries left for bottle detection. Restarting app...");
                                capture.restart_app()?;
                                app_state = AppState::WaitingToPressStart {
                                    trigger_at: Instant::now() + START_WAIT,
                                };
                                continue;
                            }

                            let next_retries_remaining = *retries_remaining - 1;
                            println!(
                                "Could not detect bottles: {:?}. Retrying in 1 second ({} retries left)...",
                                error, next_retries_remaining
                            );

                            app_state = AppState::DetectAndPlan {
                                trigger_at: now + BOTTLE_DETECTION_RETRY_DELAY,
                                retries_remaining: next_retries_remaining,
                            };
                        }
                    }
                }
            }
            AppState::AwaitPostDetectionPlan {
                trigger_at,
                detected_bottles,
            } => {
                if now >= *trigger_at {
                    let detected_bottles = detected_bottles.clone();
                    let layout = require_active_layout(&active_layout, "post-detection planning")?;
                    discovery_capture =
                        maybe_start_discovery_capture(&frame_raw, layout, &detected_bottles);

                    // Check if there are any mystery colors
                    let mystery_count = discovery::count_total_mystery_colors(&detected_bottles);
                    let hidden_count = count_hidden_bottles(&detected_bottles);
                    if mystery_count == 0 && hidden_count == 0 {
                        println!("No mystery colors detected, running solver directly...");

                        maybe_set_resolved_bottles(&mut discovery_capture, &detected_bottles);
                        finalize_discovery_capture(&mut discovery_capture);

                        let solution = solve_with_visualization(
                            &detected_bottles,
                            &frame_raw,
                            &mut window,
                            width,
                            height,
                        )?;

                        app_state = AppState::ExecuteFinalSolveMoves {
                            planned_moves: solution,
                            performed_moves: 0,
                            next_move_at: Instant::now() + MOVE_DELAY,
                        };
                    } else if mystery_count > 0 {
                        println!(
                            "Detected {} mystery colors, starting discovery process...",
                            mystery_count
                        );

                        app_state = AppState::MysteryDiscoverColors {
                            trigger_at: now,
                            initial_state: detected_bottles.clone(),
                            max_revealed_bottle_state: detected_bottles.clone(),
                            current_moves: vec![],
                        };
                    } else {
                        println!(
                            "Detected {} hidden bottle(s), starting unlock discovery...",
                            hidden_count
                        );

                        app_state = AppState::HiddenDiscoverBottles {
                            trigger_at: now,
                            current_moves: vec![],
                        };
                    }
                }
            }
            AppState::HiddenDiscoverBottles {
                trigger_at,
                current_moves,
            } => {
                if now >= *trigger_at {
                    let layout = require_active_layout(&active_layout, "hidden bottle discovery")?;

                    let current_bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, layout);

                    if let Err(error) = current_bottles {
                        println!(
                            "Error detecting bottles during hidden bottle discovery: {:?}",
                            error
                        );
                        println!("Restarting app and hoping for the best...");

                        capture.restart_app()?;
                        app_state = AppState::WaitingToPressStart {
                            trigger_at: Instant::now() + START_WAIT,
                        };

                        continue;
                    }

                    let current_bottles = current_bottles.unwrap();
                    latest_detected_bottles = Some(current_bottles.clone());
                    let mystery_count = count_total_mystery_colors(&current_bottles);
                    let hidden_count = count_hidden_bottles(&current_bottles);

                    if hidden_count == 0 {
                        if mystery_count > 0 {
                            println!(
                                "Hidden bottles unlocked and {} mystery colors remain. Starting mystery discovery...",
                                mystery_count
                            );

                            app_state = AppState::MysteryDiscoverColors {
                                trigger_at: now,
                                initial_state: current_bottles.clone(),
                                max_revealed_bottle_state: current_bottles.clone(),
                                current_moves: vec![],
                            };
                        } else {
                            println!("All hidden bottles revealed! Running solver...");

                            maybe_set_resolved_bottles(&mut discovery_capture, &current_bottles);
                            finalize_discovery_capture(&mut discovery_capture);

                            let solution = solve_with_visualization(
                                &current_bottles,
                                &frame_raw,
                                &mut window,
                                width,
                                height,
                            )?;

                            app_state = AppState::ExecuteFinalSolveMoves {
                                planned_moves: solution,
                                performed_moves: 0,
                                next_move_at: Instant::now() + MOVE_DELAY,
                            };
                        }
                    } else if mystery_count > 0 {
                        println!(
                            "Hidden bottles are still locked, but {} mystery colors remain. Switching to mystery discovery first...",
                            mystery_count
                        );

                        app_state = AppState::MysteryDiscoverColors {
                            trigger_at: now,
                            initial_state: current_bottles.clone(),
                            max_revealed_bottle_state: current_bottles.clone(),
                            current_moves: vec![],
                        };
                    } else {
                        #[cfg(feature = "discovery-debugging")]
                        {
                            let buffer = frame_to_window_buffer(&frame_display)?;
                            window.update_with_buffer(&buffer, width, height)?;

                            println!("Press enter to continue hidden-bottle discovery...");
                            std::io::stdin().read_line(&mut String::new()).unwrap();
                        }

                        match find_best_hidden_unlock_moves(&current_bottles) {
                            discovery::DiscoverResult::MoveToDiscover(best_moves) => {
                                println!("Best hidden unlock sequence found: {:?}", best_moves);
                                app_state = AppState::HiddenExecuteDiscoverMove {
                                    moves_to_execute: best_moves,
                                    current_moves: current_moves.clone(),
                                    trigger_at: now,
                                };
                            }
                            discovery::DiscoverResult::NoMove => {
                                println!(
                                    "No move found that unlocks hidden bottles. Retrying level..."
                                );

                                capture.click_at_position(RETRY_BUTTON_POS)?;

                                app_state = AppState::HiddenDiscoverBottles {
                                    trigger_at: Instant::now() + DISCOVERY_MOVE_DELAY,
                                    current_moves: vec![],
                                };
                            }
                            discovery::DiscoverResult::AlreadySolved => {
                                println!(
                                    "A hidden bottle requirement is already satisfied. Waiting for reveal..."
                                );

                                app_state = AppState::HiddenDiscoverBottles {
                                    trigger_at: Instant::now() + DISCOVERY_MOVE_DELAY,
                                    current_moves: current_moves.clone(),
                                };
                            }
                        }
                    }
                }
            }
            AppState::MysteryDiscoverColors {
                trigger_at,
                initial_state,
                max_revealed_bottle_state,
                current_moves,
            } => {
                if now >= *trigger_at {
                    let layout = require_active_layout(&active_layout, "discovery move execution")?;

                    let current_bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, layout);

                    if let Err(error) = current_bottles {
                        println!(
                            "Error detecting bottles during discovery process: {:?}",
                            error
                        );

                        println!("Restarting app and hoping for the best...");
                        capture.restart_app()?;
                        app_state = AppState::WaitingToPressStart {
                            trigger_at: Instant::now() + START_WAIT,
                        };

                        continue;
                    }

                    let mut current_bottles = current_bottles.unwrap();

                    draw_revealed_fill_markers(
                        &mut frame_display,
                        layout,
                        &current_bottles,
                        max_revealed_bottle_state,
                    )?;

                    improve_best_revealed_state(
                        max_revealed_bottle_state,
                        initial_state,
                        &current_bottles,
                    );
                    improve_current_bottles_with_revealed_state(
                        &mut current_bottles,
                        max_revealed_bottle_state,
                    );
                    latest_detected_bottles = Some(current_bottles.clone());

                    let mystery_colors = count_total_mystery_colors(max_revealed_bottle_state);
                    println!("Total mystery colors still hidden: {}", mystery_colors);
                    if mystery_colors == 0 {
                        let hidden_count = count_hidden_bottles(max_revealed_bottle_state);
                        if hidden_count > 0 {
                            println!(
                                "All mystery colors revealed, but {} hidden bottle(s) remain locked. Switching to hidden discovery...",
                                hidden_count
                            );

                            app_state = AppState::HiddenDiscoverBottles {
                                trigger_at: now,
                                current_moves: current_moves.clone(),
                            };
                            continue;
                        }

                        println!("All mystery colors revealed! Running solver...");

                        maybe_set_resolved_bottles(
                            &mut discovery_capture,
                            max_revealed_bottle_state,
                        );

                        finalize_discovery_capture(&mut discovery_capture);

                        let mut solver_bottles = Vec::new();
                        max_revealed_bottle_state
                            .iter()
                            .enumerate()
                            .for_each(|(i, bottle)| {
                                solver_bottles.push(Bottle::from_fills_with_initial(
                                    bottle.get_fills().clone(),
                                    initial_state[i].get_fills().clone(),
                                ));
                            });

                        let solution = solve_with_visualization(
                            &solver_bottles,
                            &frame_raw,
                            &mut window,
                            width,
                            height,
                        )?;

                        println!("Resetting level for the solver...");
                        capture.click_at_position(RETRY_BUTTON_POS)?;
                        app_state = AppState::ExecuteFinalSolveMoves {
                            planned_moves: solution,
                            performed_moves: 0,
                            next_move_at: Instant::now() + MOVE_DELAY,
                        };
                    } else {
                        #[cfg(feature = "discovery-debugging")]
                        {
                            let buffer = frame_to_window_buffer(&frame_display)?;
                            window.update_with_buffer(&buffer, width, height)?;

                            println!("Press enter to continue discovery...");
                            std::io::stdin().read_line(&mut String::new()).unwrap();
                        }

                        // Find best move to reveal more colors
                        let best_move =
                            find_best_discovery_moves(&current_bottles, max_revealed_bottle_state);

                        match best_move {
                            discovery::DiscoverResult::MoveToDiscover(best_moves) => {
                                println!("Best discovery move sequence found: {:?}", best_moves);
                                app_state = AppState::MysteryExecuteDiscoverMove {
                                    moves_to_execute: best_moves,
                                    initial_state: initial_state.clone(),
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    current_moves: current_moves.clone(),
                                    trigger_at: now,
                                };
                            }
                            discovery::DiscoverResult::NoMove => {
                                println!(
                                    "No discovery move found that reveals new colors. Retrying level..."
                                );

                                capture.click_at_position(RETRY_BUTTON_POS)?;

                                app_state = AppState::MysteryDiscoverColors {
                                    trigger_at: Instant::now() + DISCOVERY_MOVE_DELAY,
                                    initial_state: initial_state.clone(),
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    current_moves: vec![],
                                };
                            }
                            discovery::DiscoverResult::AlreadySolved => {
                                let hidden_count = count_hidden_bottles(max_revealed_bottle_state);
                                if hidden_count > 0 {
                                    println!(
                                        "Mystery discovery finished, but {} hidden bottle(s) remain locked. Switching to hidden discovery...",
                                        hidden_count
                                    );

                                    app_state = AppState::HiddenDiscoverBottles {
                                        trigger_at: now,
                                        current_moves: current_moves.clone(),
                                    };
                                } else {
                                    println!(
                                        "While discovering, the puzzle has been solved. Proceeding to next level..."
                                    );

                                    maybe_set_resolved_bottles(
                                        &mut discovery_capture,
                                        max_revealed_bottle_state,
                                    );
                                    finalize_discovery_capture(&mut discovery_capture);

                                    app_state = AppState::CheckForRewards {
                                        trigger_at: now + NEXT_LEVEL_WAIT,
                                    };
                                }
                            }
                        }
                    }
                }
            }
            AppState::HiddenExecuteDiscoverMove {
                trigger_at,
                moves_to_execute,
                current_moves,
            } => {
                if now >= *trigger_at {
                    let layout =
                        require_active_layout(&active_layout, "hidden bottle move execution")?;

                    let current_bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, layout);

                    if let Err(error) = current_bottles {
                        return Err(anyhow!(
                            "Error detecting bottles during hidden bottle move execution: {:?}",
                            error
                        ));
                    }

                    let current_bottles = current_bottles.unwrap();
                    latest_detected_bottles = Some(current_bottles.clone());

                    if moves_to_execute.is_empty() {
                        app_state = AppState::HiddenDiscoverBottles {
                            trigger_at: now,
                            current_moves: current_moves.clone(),
                        };
                    } else {
                        let next_move = moves_to_execute.remove(0);
                        let reveal_wait_needed =
                            move_satisfies_hidden_requirement(&current_bottles, next_move);

                        if !next_move.can_perform_on_bottles(&current_bottles) {
                            return Err(anyhow!(
                                "Planned hidden-bottle move cannot be performed on the currently detected bottle state. This should not happen. Move: {:?}, Detected bottles: {:?}",
                                next_move,
                                current_bottles
                            ));
                        }

                        println!("Performing hidden-bottle move: {:?}.", next_move);
                        #[cfg(feature = "discovery-debugging")]
                        {
                            println!("Press enter to perform the next move...");
                            std::io::stdin().read_line(&mut String::new()).unwrap();
                        }
                        next_move.perform_move_on_device(layout, &capture)?;

                        current_moves.push(next_move);
                        *trigger_at = Instant::now()
                            + if reveal_wait_needed {
                                HIDDEN_REVEAL_DETECTION_DELAY
                            } else {
                                DISCOVERY_MOVE_DELAY
                            };
                    }
                }
            }
            AppState::MysteryExecuteDiscoverMove {
                trigger_at,
                moves_to_execute,
                max_revealed_bottle_state,
                current_moves,
                initial_state,
            } => {
                if now >= *trigger_at {
                    let layout = require_active_layout(&active_layout, "discovery move execution")?;

                    let current_bottles =
                        detect_bottles_with_layout(&frame_raw, &mut frame_display, layout);

                    if let Err(error) = current_bottles {
                        {
                            // Saving current state for debugging
                            let timestamp = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs();
                            // Save current moves, visisted states everything to a string for me to debug
                            let debug_info = format!(
                                "Error: {:?}\nCurrent Moves: {:#?}\nMax Revealed Bottle State: {:#?}",
                                error, current_moves, max_revealed_bottle_state
                            );
                            std::fs::write(
                                format!("target/discovery_move_error_{}.txt", timestamp),
                                debug_info,
                            )?;
                        }
                        return Err(anyhow!(
                            "Error detecting bottles during discovery move execution: {:?}",
                            error
                        ));
                    }

                    let current_bottles = current_bottles.unwrap();
                    latest_detected_bottles = Some(current_bottles.clone());
                    draw_revealed_fill_markers(
                        &mut frame_display,
                        layout,
                        &current_bottles,
                        max_revealed_bottle_state,
                    )?;

                    if moves_to_execute.is_empty() {
                        app_state = AppState::MysteryDiscoverColors {
                            trigger_at: now,
                            initial_state: initial_state.clone(),
                            max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                            current_moves: current_moves.clone(),
                        };
                    } else {
                        let next_move = moves_to_execute.remove(0);

                        if !next_move.can_perform_on_bottles(&current_bottles) {
                            return Err(anyhow!(
                                "Planned discovery move cannot be performed on the currently detected bottle state. This should not happen. Move: {:?}, Detected bottles: {:?}",
                                next_move,
                                current_bottles
                            ));
                        }

                        println!("Performing discovery move: {:?}.", next_move);
                        #[cfg(feature = "discovery-debugging")]
                        {
                            println!("Press enter to perform the next move...");
                            std::io::stdin().read_line(&mut String::new()).unwrap();
                        }
                        next_move.perform_move_on_device(layout, &capture)?;

                        // Remove the executed move from the list
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
                        let layout = require_active_layout(&active_layout, "solve move execution")?;
                        next.perform_move_on_device(layout, &capture)?;
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
                    let has_no_thank_you = NO_THANK_YOU_POSITIONS.iter().any(|pos| {
                        let pixel = frame_raw.at_2d::<Vec3b>(pos.1, pos.0).unwrap();

                        color_distance_sq(pixel, &NO_THANK_YOU_REWARDS_COLOR) < 50 * 50
                    });

                    if has_no_thank_you {
                        println!("Reward screen detected, clicking 'No, thank you'...");
                        capture.click_at_position(NO_THANK_YOU_POSITIONS[0])?;
                    } else {
                        println!("No reward screen detected, proceeding to next level...");
                    }

                    app_state = AppState::ClickNextLevel {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                    };
                }
            }
        }

        let overlay_snapshot = build_overlay_snapshot(&app_state, now, &active_layout);

        if let (Some(layout), Some(bottles)) = (&active_layout, latest_detected_bottles.as_deref())
        {
            draw_detected_bottles_overlay(&mut frame_display, layout, bottles)?;
        }

        draw_state_hud(&mut frame_display, width, &overlay_snapshot)?;

        let buffer = frame_to_window_buffer(&frame_display)?;
        window.update_with_buffer(&buffer, width, height)?;
    }

    Ok(())
}

fn require_active_layout<'a>(
    active_layout: &'a Option<BottleLayout>,
    context: &str,
) -> Result<&'a BottleLayout> {
    active_layout
        .as_ref()
        .ok_or_else(|| anyhow!("No active layout available for {}.", context))
}

fn solve_with_visualization(
    bottles: &[Bottle],
    frame_raw: &Mat,
    window: &mut Window,
    width: usize,
    height: usize,
) -> Result<Vec<Move>> {
    #[cfg(feature = "solver-visualization")]
    {
        let baseline_frame = frame_raw.try_clone()?;
        let mut last_update = Instant::now() - SOLVER_VISUALIZATION_UPDATE_INTERVAL;

        let maybe_solution = crate::solver::run_solver_with_progress(bottles, |snapshot| {
            if !snapshot.is_goal && last_update.elapsed() < SOLVER_VISUALIZATION_UPDATE_INTERVAL {
                return;
            }
            last_update = Instant::now();

            let mut preview_frame = match baseline_frame.try_clone() {
                Ok(frame) => frame,
                Err(error) => {
                    println!("Solver visualization frame clone failed: {:?}", error);
                    return;
                }
            };

            if let Err(error) = draw_solver_search_preview(
                &mut preview_frame,
                snapshot.state,
                snapshot.explored_states,
                snapshot.queue_len,
                snapshot.depth,
                snapshot.is_goal,
            ) {
                println!("Solver visualization draw failed: {:?}", error);
                return;
            }

            match frame_to_window_buffer(&preview_frame) {
                Ok(buffer) => {
                    if let Err(error) = window.update_with_buffer(&buffer, width, height) {
                        println!("Solver visualization window update failed: {:?}", error);
                    }
                }
                Err(error) => {
                    println!("Solver visualization buffer conversion failed: {:?}", error);
                }
            }

            std::thread::sleep(SOLVER_VISUALIZATION_FRAME_DELAY);
        });

        maybe_solution.ok_or_else(|| anyhow!("Failed to find solver solution"))
    }

    #[cfg(not(feature = "solver-visualization"))]
    {
        let _ = (frame_raw, window, width, height);
        run_solver(bottles).ok_or_else(|| anyhow!("Failed to find solver solution"))
    }
}

fn remaining_until(trigger_at: Instant, now: Instant) -> Option<Duration> {
    if trigger_at > now {
        Some(trigger_at.duration_since(now))
    } else {
        None
    }
}

fn move_satisfies_hidden_requirement(current_bottles: &[Bottle], mv: Move) -> bool {
    let hidden_requirements = collect_hidden_requirements(current_bottles);
    if hidden_requirements.is_empty() || !mv.can_perform_on_bottles(current_bottles) {
        return false;
    }

    let mut simulated = current_bottles.to_vec();
    mv.perform_move_on_bottles(&mut simulated);

    simulated.iter().any(|bottle| {
        bottle
            .solved_color()
            .is_some_and(|color| hidden_requirements.contains(&color))
    })
}

fn maybe_start_discovery_capture(
    frame_raw: &Mat,
    layout: &BottleLayout,
    detected_bottles: &[Bottle],
) -> Option<DiscoveryCaptureContext> {
    #[cfg(feature = "collect-test-data")]
    {
        match start_discovery_capture(frame_raw, layout, detected_bottles) {
            Ok(capture_context) => Some(capture_context),
            Err(error) => {
                println!("Warning: Failed to start discovery capture: {:?}", error);
                None
            }
        }
    }

    #[cfg(not(feature = "collect-test-data"))]
    {
        let _ = (frame_raw, layout, detected_bottles);
        None
    }
}

fn maybe_set_resolved_bottles(
    discovery_capture: &mut Option<DiscoveryCaptureContext>,
    max_revealed_bottle_state: &[Bottle],
) {
    #[cfg(feature = "collect-test-data")]
    {
        if let Some(capture_context) = discovery_capture.as_mut() {
            capture_context.set_resolved_bottles(max_revealed_bottle_state);
        }
    }

    #[cfg(not(feature = "collect-test-data"))]
    {
        let _ = (discovery_capture, max_revealed_bottle_state);
    }
}

fn finalize_discovery_capture(discovery_capture: &mut Option<DiscoveryCaptureContext>) {
    #[cfg(not(feature = "collect-test-data"))]
    {
        let _ = discovery_capture;
    }

    #[cfg(feature = "collect-test-data")]
    {
        let Some(capture_context) = discovery_capture.take() else {
            return;
        };

        if let Err(error) = capture_context.finalize() {
            println!(
                "Warning: Failed to persist discovery capture manifest entry: {:?}",
                error
            );
        }
    }
}

fn build_overlay_snapshot<'a>(
    app_state: &'a AppState,
    now: Instant,
    #[cfg(feature = "solver-visualization")] active_layout: &'a Option<BottleLayout>,
    #[cfg(not(feature = "solver-visualization"))] _active_layout: &'a Option<BottleLayout>,
) -> OverlaySnapshot<'a> {
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
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
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
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
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
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::DetectAndPlan {
            trigger_at,
            retries_remaining,
        } => OverlaySnapshot {
            phase: "DetectAndPlan".to_string(),
            detail: if *retries_remaining == BOTTLE_DETECTION_RETRIES {
                "Detecting bottle layout and planning".to_string()
            } else {
                format!(
                    "Retrying bottle detection ({} retries left)",
                    *retries_remaining
                )
            },
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::AwaitPostDetectionPlan { trigger_at, .. } => OverlaySnapshot {
            phase: "AwaitPostDetectionPlan".to_string(),
            detail: "Reviewing detected bottles before planning".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::HiddenDiscoverBottles {
            trigger_at,
            current_moves,
        } => OverlaySnapshot {
            phase: "HiddenDiscoverBottles".to_string(),
            detail: "Scanning bottles to unlock hidden slots".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(0),
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::HiddenExecuteDiscoverMove {
            trigger_at,
            moves_to_execute,
            current_moves,
        } => OverlaySnapshot {
            phase: "HiddenExecuteDiscoverMove".to_string(),
            detail: "Executing hidden-slot unlock sequence".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(moves_to_execute.len()),
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
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
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
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
            #[cfg(feature = "solver-visualization")]
            solve_layout: None,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
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
            #[cfg(feature = "solver-visualization")]
            solve_layout: active_layout.as_ref(),
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: *performed_moves,
        },
    }
}
