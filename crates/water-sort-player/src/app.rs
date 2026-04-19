use std::{
    collections::HashSet,
    collections::VecDeque,
    path::Path,
    time::{Duration, Instant},
};

mod frame_stability;

#[cfg(feature = "save-states")]
mod save_states;

use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};
use minifb::{MouseButton, MouseMode, Window, WindowOptions};
use opencv::{
    core::{Mat, MatTraitConst, Vec3b, Vector},
    imgcodecs,
};
#[cfg(feature = "save-states")]
use save_states::SaveStatesRecorder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use water_sort_core::{
    BottleColor,
    constants::{
        NEXT_LEVEL_BUTTON_COLOR, NEXT_LEVEL_BUTTON_POSITIONS, NO_THANK_YOU_POSITIONS,
        NO_THANK_YOU_REWARDS_COLOR, color_distance_sq, reset_color_label_assignments,
    },
};
use water_sort_device::{CaptureDeviceBackend, construct_capture_backend};
use water_sort_solver::build_solver_initial_bottle_state;

use self::frame_stability::{
    MotionWindowState, evaluate_motion_window, frames_are_identical, has_no_movement_in_window,
};
use crate::{
    app_visualization::{OverlaySnapshot, draw_detected_bottles_overlay, draw_state_hud},
    bottles::{Bottle, detect_bottles},
    capture::{DiscoveryCaptureContext, frame_to_window_buffer, save_frame_png},
    constants::{RETRY_BUTTON_POS, START_BUTTON_POS},
    solver::{
        Move,
        discovery::{
            self, count_hidden_bottles, count_total_mystery_colors, find_best_discovery_moves,
            find_best_hidden_unlock_moves, improve_best_revealed_state,
            improve_current_and_initial_bottles_with_revealed_state,
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
const NO_MOVEMENT_WINDOW: Duration = Duration::from_millis(1000);
const FRAME_SIMILARITY_MEAN_DIFF_THRESHOLD: f64 = 0.1;
#[cfg(feature = "solver-visualization")]
const SOLVER_VISUALIZATION_UPDATE_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(feature = "solver-visualization")]
const SOLVER_VISUALIZATION_FRAME_DELAY: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum AppState {
    WaitingToPressStart {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
    },
    ClickNextLevel {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
    },
    ClickRetryOnNewLevel {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
    },
    CheckForRewards {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
    },
    DetectAndPlan {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
        retries_remaining: u8,
    },
    AwaitPostDetectionPlan {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
        detected_bottles: Vec<Bottle>,
        known_colors: HashSet<BottleColor>,
    },
    HiddenDiscoverBottles {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
        max_revealed_bottle_state: Vec<Bottle>,
        initial_state: Vec<Bottle>,
        known_colors: HashSet<BottleColor>,
        current_moves: Vec<Move>,
        force_hidden_discovery: bool,
        hidden_level_retried: bool,
        retries_remaining: u8,
    },
    HiddenExecuteDiscoverMove {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
        max_revealed_bottle_state: Vec<Bottle>,
        initial_state: Vec<Bottle>,
        known_colors: HashSet<BottleColor>,
        moves_to_execute: Vec<Move>,
        current_moves: Vec<Move>,
        force_hidden_discovery: bool,
        hidden_level_retried: bool,
        retries_remaining: u8,
    },
    MysteryDiscoverColors {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
        initial_state: Vec<Bottle>,
        known_colors: HashSet<BottleColor>,
        max_revealed_bottle_state: Vec<Bottle>,
        current_moves: Vec<Move>,
        mystery_level_retried: bool,
        retries_remaining: u8,
    },
    MysteryExecuteDiscoverMove {
        #[serde(skip, default = "instant_now")]
        trigger_at: Instant,
        moves_to_execute: Vec<Move>,
        initial_state: Vec<Bottle>,
        known_colors: HashSet<BottleColor>,
        max_revealed_bottle_state: Vec<Bottle>,
        current_moves: Vec<Move>,
        mystery_level_retried: bool,
        retries_remaining: u8,
    },
    ExecutePlanSolverMoves {
        current_bottles: Vec<Bottle>,
        max_revealed_bottle_state: Vec<Bottle>,
        initial_state: Vec<Bottle>,
        known_colors: HashSet<BottleColor>,
    },
    ExecuteFinalSolveMoves {
        #[serde(skip, default = "instant_now")]
        next_move_at: Instant,
        planned_moves: Vec<Move>,
        performed_moves: usize,
        known_colors: HashSet<BottleColor>,
    },
}

fn instant_now() -> Instant {
    Instant::now()
}

impl AppState {
    fn get_name(&self) -> String {
        match self {
            AppState::WaitingToPressStart { .. } => "WaitingToPressStart",
            AppState::ClickNextLevel { .. } => "ClickNextLevel",
            AppState::ClickRetryOnNewLevel { .. } => "ClickRetryOnNewLevel",
            AppState::CheckForRewards { .. } => "CheckForRewards",
            AppState::DetectAndPlan { .. } => "DetectAndPlan",
            AppState::AwaitPostDetectionPlan { .. } => "AwaitPostDetectionPlan",
            AppState::HiddenDiscoverBottles { .. } => "HiddenDiscoverBottles",
            AppState::HiddenExecuteDiscoverMove { .. } => "HiddenExecuteDiscoverMove",
            AppState::MysteryDiscoverColors { .. } => "MysteryDiscoverColors",
            AppState::MysteryExecuteDiscoverMove { .. } => "MysteryExecuteDiscoverMove",
            AppState::ExecuteFinalSolveMoves { .. } => "ExecuteFinalSolveMoves",
            AppState::ExecutePlanSolverMoves { .. } => "ExecutePlanSolverMoves",
        }
        .to_string()
    }
}

pub fn run(quick_mode: bool, use_state_path: Option<&Path>) -> Result<()> {
    if quick_mode {
        info!("Quick start mode enabled: skipping scrcpy startup and start-button automation.");
    }
    #[cfg(feature = "collect-test-data")]
    info!("Test data collection enabled (feature: collect-test-data).");

    let mut capture = construct_capture_backend();
    let (width, height) = capture.start_capture(quick_mode)?;
    info!("Creating window...");
    let mut window = Window::new("AutoPlayer", width, height, WindowOptions::default())?;
    let mut frame_raw: Mat;

    let mut app_state = if let Some(path) = use_state_path {
        info!("Loading app state from {}...", path.display());
        let loaded_state = load_app_state_from_file(path)?;
        info!("Loaded initial app state: {}", loaded_state.get_name());
        loaded_state
    } else if quick_mode {
        AppState::ClickRetryOnNewLevel {
            trigger_at: Instant::now() + Duration::from_secs(1),
        }
    } else {
        AppState::WaitingToPressStart {
            trigger_at: Instant::now() + START_WAIT,
        }
    };
    let mut previous_right_click = false;
    let mut latest_detected_bottles: Option<Vec<Bottle>> = None;
    let mut discovery_capture: Option<DiscoveryCaptureContext> = None;
    #[cfg(feature = "save-states")]
    let mut state_capture = SaveStatesRecorder::new()?;

    let mut first_frame_read = true;
    let mut prev_app_state = app_state.clone();
    let mut previous_frame_raw: Option<Mat> = None;
    let mut recent_frame_matches: VecDeque<(Instant, bool)> = VecDeque::new();
    while window.is_open() {
        if first_frame_read {
            debug!("Reading first frame...");
            first_frame_read = false;
        }
        match capture.capture_frame() {
            Ok(frame) => frame_raw = frame,
            Err(error) => {
                warn!("Skipping frame read error: {:?}", error);
                continue;
            }
        }

        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp)
            && window.get_mouse_down(MouseButton::Left)
        {
            debug!("Clicked at: ({}, {})", x, y);
        }

        let right_click = window.get_mouse_down(MouseButton::Right);
        if right_click && !previous_right_click {
            let saved_path = save_frame_png(&frame_raw)?;
            info!("Saved quick-iteration frame to {}", saved_path.display());
        }
        previous_right_click = right_click;

        let now = Instant::now();
        let frames_match_previous = if let Some(previous_frame) = previous_frame_raw.as_ref() {
            match frames_are_identical(
                previous_frame,
                &frame_raw,
                FRAME_SIMILARITY_MEAN_DIFF_THRESHOLD,
            ) {
                Ok(matches) => matches,
                Err(error) => {
                    warn!(
                        "Failed to compare frames for movement detection: {:?}",
                        error
                    );
                    false
                }
            }
        } else {
            false
        };

        recent_frame_matches.push_back((now, frames_match_previous));
        if let Some(cutoff) = now.checked_sub(NO_MOVEMENT_WINDOW) {
            while recent_frame_matches
                .front()
                .is_some_and(|(timestamp, _)| *timestamp < cutoff)
            {
                recent_frame_matches.pop_front();
            }
        }

        let frame_is_still =
            has_no_movement_in_window(&recent_frame_matches, now, NO_MOVEMENT_WINDOW);
        let motion_status_text = format_motion_status(evaluate_motion_window(
            &recent_frame_matches,
            now,
            NO_MOVEMENT_WINDOW,
        ));
        previous_frame_raw = Some(frame_raw.try_clone()?);

        let mut frame_display = frame_raw.try_clone()?;

        if frame_is_still {
            match &mut app_state {
                AppState::WaitingToPressStart { trigger_at } => {
                    if now >= *trigger_at {
                        info!("Starting level...");
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
                            debug!("Checking for next level button at position {:?} with pixel value {:?}...", pos, pixel);
                            debug!("Color distance to expected next level button color: {}", color_distance_sq(pixel, &NEXT_LEVEL_BUTTON_COLOR));
                            debug!("Hex value of pixel: #{:02x}{:02x}{:02x}", pixel[2], pixel[1], pixel[0]);
                            color_distance_sq(pixel, &NEXT_LEVEL_BUTTON_COLOR)
                                <= 50 * 50
                        })
                        .copied()
                        .unwrap_or(NEXT_LEVEL_BUTTON_POSITIONS[0]);
                        info!("Pressing next level button at {button_pos:?}...");

                        capture.click_at_position(button_pos)?;
                        app_state = AppState::ClickRetryOnNewLevel {
                            trigger_at: now + NEXT_LEVEL_WAIT,
                        };
                    }
                }
                AppState::ClickRetryOnNewLevel { trigger_at } => {
                    if now >= *trigger_at {
                        info!("Clicking retry button for new level...");
                        capture.click_at_position(RETRY_BUTTON_POS)?;

                        app_state = AppState::DetectAndPlan {
                            trigger_at: now,
                            retries_remaining: BOTTLE_DETECTION_RETRIES,
                        };
                    }
                }
                AppState::DetectAndPlan {
                    trigger_at,
                    retries_remaining,
                } => {
                    if now >= *trigger_at && frame_is_still {
                        info!("Detecting bottles for new level...");
                        let mut known_colors = HashSet::new();
                        let bottles =
                            detect_bottles(&frame_raw, &mut frame_display, &mut known_colors);
                        match bottles {
                            Ok(detected_bottles) => {
                                latest_detected_bottles = Some(detected_bottles.clone());
                                app_state = AppState::AwaitPostDetectionPlan {
                                    trigger_at: now + POST_DETECTION_WAIT,
                                    detected_bottles,
                                    known_colors,
                                };
                            }
                            Err(error) => {
                                if *retries_remaining == 0 {
                                    warn!(
                                        "No retries left for bottle detection. Restarting app..."
                                    );
                                    capture.restart_app()?;
                                    app_state = AppState::WaitingToPressStart {
                                        trigger_at: Instant::now() + START_WAIT,
                                    };
                                    continue;
                                }

                                let next_retries_remaining = *retries_remaining - 1;
                                warn!(
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
                    known_colors,
                } => {
                    if now >= *trigger_at {
                        let detected_bottles = detected_bottles.clone();
                        discovery_capture =
                            maybe_start_discovery_capture(&frame_raw, &detected_bottles);

                        // Check if there are any mystery colors
                        let mystery_count =
                            discovery::count_total_mystery_colors(&detected_bottles);
                        let hidden_count = count_hidden_bottles(&detected_bottles);
                        if mystery_count == 0 && hidden_count == 0 {
                            info!("No mystery colors detected, running solver directly...");
                            app_state = AppState::ExecutePlanSolverMoves {
                                current_bottles: detected_bottles.clone(),
                                max_revealed_bottle_state: detected_bottles.clone(),
                                initial_state: detected_bottles.clone(),
                                known_colors: known_colors.clone(),
                            };
                        } else if mystery_count > 0 {
                            info!(
                                "Detected {} mystery colors, starting discovery process...",
                                mystery_count
                            );

                            //opencv::imgcodecs::imwrite("initial_detected_bottles.png", &frame_raw, &Vector::new())?;
                            log::debug!(
                                "Initial detected bottles: {}",
                                detected_bottles
                                    .iter()
                                    .map(|b| b.to_string())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            );

                            app_state = AppState::MysteryDiscoverColors {
                                trigger_at: now,
                                initial_state: detected_bottles.clone(),
                                max_revealed_bottle_state: detected_bottles.clone(),
                                known_colors: known_colors.clone(),
                                current_moves: vec![],
                                mystery_level_retried: false,
                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                            };
                        } else {
                            info!(
                                "Detected {} hidden bottle(s), starting unlock discovery...",
                                hidden_count
                            );

                            app_state = AppState::HiddenDiscoverBottles {
                                trigger_at: now,
                                initial_state: detected_bottles.clone(),
                                max_revealed_bottle_state: detected_bottles.clone(),
                                current_moves: vec![],
                                known_colors: known_colors.clone(),
                                force_hidden_discovery: false,
                                hidden_level_retried: false,
                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                            };
                        }
                    }
                }
                AppState::HiddenDiscoverBottles {
                    trigger_at,
                    current_moves,
                    initial_state,
                    max_revealed_bottle_state,
                    force_hidden_discovery,
                    hidden_level_retried,
                    retries_remaining,
                    known_colors,
                } => {
                    if now >= *trigger_at {
                        log::debug!(
                            "Max revealed {}",
                            max_revealed_bottle_state
                                .iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                        let current_bottles =
                            detect_bottles(&frame_raw, &mut frame_display, known_colors);

                        if let Err(error) = current_bottles {
                            if *retries_remaining == 0 {
                                warn!(
                                    "Error detecting bottles during hidden bottle discovery: {:?}",
                                    error
                                );
                                warn!("Restarting app and hoping for the best...");

                                capture.restart_app()?;
                                app_state = AppState::WaitingToPressStart {
                                    trigger_at: Instant::now() + START_WAIT,
                                };

                                continue;
                            }

                            let next_retries_remaining = *retries_remaining - 1;
                            warn!(
                                "Could not detect bottles during hidden bottle discovery: {:?}. Retrying in 1 second ({} retries left)...",
                                error, next_retries_remaining
                            );

                            app_state = AppState::HiddenDiscoverBottles {
                                trigger_at: now + BOTTLE_DETECTION_RETRY_DELAY,
                                current_moves: current_moves.clone(),
                                initial_state: initial_state.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                force_hidden_discovery: *force_hidden_discovery,
                                hidden_level_retried: *hidden_level_retried,
                                retries_remaining: next_retries_remaining,
                                known_colors: known_colors.clone(),
                            };
                            continue;
                        }

                        let mut current_bottles = current_bottles.unwrap();
                        improve_current_and_initial_bottles_with_revealed_state(
                            &mut current_bottles,
                            initial_state,
                            max_revealed_bottle_state,
                        );
                        improve_best_revealed_state(
                            max_revealed_bottle_state,
                            initial_state,
                            &current_bottles,
                        );

                        latest_detected_bottles = Some(current_bottles.clone());

                        let mystery_count = count_total_mystery_colors(max_revealed_bottle_state);
                        let hidden_count = count_hidden_bottles(max_revealed_bottle_state);

                        log::trace!(
                            "Mystery {mystery_count}, hidden {hidden_count}, forced: {force_hidden_discovery}"
                        );
                        if hidden_count == 0 {
                            if mystery_count > 0 {
                                info!(
                                    "Hidden bottles unlocked and {} mystery colors remain. Starting mystery discovery...",
                                    mystery_count
                                );

                                app_state = AppState::MysteryDiscoverColors {
                                    trigger_at: now,
                                    initial_state: initial_state.clone(),
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    current_moves: vec![],
                                    known_colors: known_colors.clone(),
                                    mystery_level_retried: false,
                                    retries_remaining: BOTTLE_DETECTION_RETRIES,
                                };
                            } else {
                                info!(
                                    "All hidden bottles revealed! Running solver, Mystery count: {mystery_count}, hidden count: {hidden_count}..."
                                );

                                log::debug!(
                                    "Final revealed state before solving: {}",
                                    max_revealed_bottle_state
                                        .iter()
                                        .map(|b| b.to_string())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                );

                                app_state = AppState::ExecutePlanSolverMoves {
                                    current_bottles: current_bottles.clone(),
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    initial_state: initial_state.clone(),
                                    known_colors: known_colors.clone(),
                                };
                            }
                        } else if mystery_count > 0 && !*force_hidden_discovery {
                            info!(
                                "Hidden bottles are still locked, but {} mystery colors remain. Switching to mystery discovery first...",
                                mystery_count
                            );

                            app_state = AppState::MysteryDiscoverColors {
                                trigger_at: now,
                                initial_state: initial_state.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                current_moves: vec![],
                                known_colors: known_colors.clone(),
                                mystery_level_retried: false,
                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                            };
                        } else {
                            let moves = find_best_hidden_unlock_moves(&current_bottles);
                            #[cfg(feature = "discovery-debugging")]
                            {
                                let buffer = frame_to_window_buffer(&frame_display)?;
                                window.update_with_buffer(&buffer, width, height)?;

                                info!("Press enter to continue hidden-bottle discovery...");
                                std::io::stdin().read_line(&mut String::new()).unwrap();
                            }

                            match moves {
                                discovery::DiscoverResult::MoveToDiscover(best_moves) => {
                                    info!(
                                        "Best hidden unlock sequence found: {}",
                                        best_moves
                                            .iter()
                                            .map(|m| m.to_string())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    );
                                    app_state = AppState::HiddenExecuteDiscoverMove {
                                        moves_to_execute: best_moves,
                                        initial_state: initial_state.clone(),
                                        current_moves: current_moves.clone(),
                                        known_colors: known_colors.clone(),
                                        trigger_at: now,
                                        max_revealed_bottle_state: max_revealed_bottle_state
                                            .clone(),
                                        force_hidden_discovery: *force_hidden_discovery,
                                        hidden_level_retried: *hidden_level_retried,
                                        retries_remaining: BOTTLE_DETECTION_RETRIES,
                                    };
                                }
                                discovery::DiscoverResult::NoMove => {
                                    if *hidden_level_retried {
                                        let mystery_count =
                                            count_total_mystery_colors(max_revealed_bottle_state);

                                        if mystery_count > 0 {
                                            info!(
                                                "No hidden unlock move found after retry; returning to mystery discovery..."
                                            );
                                            app_state = AppState::MysteryDiscoverColors {
                                                known_colors: known_colors.clone(),
                                                trigger_at: now,
                                                initial_state: initial_state.clone(),
                                                max_revealed_bottle_state:
                                                    max_revealed_bottle_state.clone(),
                                                current_moves: vec![],
                                                mystery_level_retried: true,
                                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                                            };
                                        } else {
                                            warn!(
                                                "No hidden unlock move found after retry and no mystery colors remain. Retrying level..."
                                            );

                                            capture.click_at_position(RETRY_BUTTON_POS)?;

                                            app_state = AppState::HiddenDiscoverBottles {
                                                trigger_at: Instant::now(),
                                                initial_state: initial_state.clone(),
                                                current_moves: vec![],
                                                known_colors: known_colors.clone(),
                                                max_revealed_bottle_state:
                                                    max_revealed_bottle_state.clone(),
                                                force_hidden_discovery: *force_hidden_discovery,
                                                hidden_level_retried: false,
                                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                                            };
                                        }
                                    } else {
                                        warn!(
                                            "No move found that unlocks hidden bottles. Retrying level..."
                                        );

                                        capture.click_at_position(RETRY_BUTTON_POS)?;

                                        app_state = AppState::HiddenDiscoverBottles {
                                            trigger_at: Instant::now(),
                                            initial_state: initial_state.clone(),
                                            current_moves: vec![],
                                            known_colors: known_colors.clone(),
                                            max_revealed_bottle_state: max_revealed_bottle_state
                                                .clone(),
                                            force_hidden_discovery: *force_hidden_discovery,
                                            hidden_level_retried: true,
                                            retries_remaining: BOTTLE_DETECTION_RETRIES,
                                        };
                                    }
                                }
                                discovery::DiscoverResult::AlreadySolved => {
                                    info!(
                                        "A hidden bottle requirement is already satisfied. Waiting for reveal..."
                                    );

                                    debug!(
                                        "Current bottles at already solved state: {}",
                                        current_bottles
                                            .iter()
                                            .map(|b| b.to_string())
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    );

                                    debug!(
                                        "Max revealed bottle state at already solved state: {}",
                                        max_revealed_bottle_state
                                            .iter()
                                            .map(|b| b.to_string())
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                    );

                                    app_state = AppState::HiddenDiscoverBottles {
                                        trigger_at: Instant::now(),
                                        initial_state: initial_state.clone(),
                                        known_colors: known_colors.clone(),
                                        current_moves: current_moves.clone(),
                                        max_revealed_bottle_state: max_revealed_bottle_state
                                            .clone(),
                                        force_hidden_discovery: *force_hidden_discovery,
                                        hidden_level_retried: *hidden_level_retried,
                                        retries_remaining: BOTTLE_DETECTION_RETRIES,
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
                    mystery_level_retried,
                    retries_remaining,
                    known_colors,
                } => {
                    if now >= *trigger_at {
                        log::debug!(
                            "Max revealed {}",
                            max_revealed_bottle_state
                                .iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                        let current_bottles =
                            detect_bottles(&frame_raw, &mut frame_display, known_colors);

                        if let Err(error) = current_bottles {
                            if *retries_remaining == 0 {
                                warn!(
                                    "Error detecting bottles during discovery process: {:?}",
                                    error
                                );

                                warn!("Restarting app and hoping for the best...");
                                capture.restart_app()?;
                                app_state = AppState::WaitingToPressStart {
                                    trigger_at: Instant::now() + START_WAIT,
                                };
                                continue;
                            }

                            let next_retries_remaining = *retries_remaining - 1;
                            warn!(
                                "Could not detect bottles during discovery process: {:?}. Retrying in 1 second ({} retries left)...",
                                error, next_retries_remaining
                            );

                            app_state = AppState::MysteryDiscoverColors {
                                trigger_at: now + BOTTLE_DETECTION_RETRY_DELAY,
                                initial_state: initial_state.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                current_moves: current_moves.clone(),
                                known_colors: known_colors.clone(),
                                mystery_level_retried: *mystery_level_retried,
                                retries_remaining: next_retries_remaining,
                            };
                            continue;
                        }

                        let mut current_bottles = current_bottles.unwrap();

                        draw_revealed_fill_markers(
                            &mut frame_display,
                            &current_bottles,
                            max_revealed_bottle_state,
                        )?;

                        improve_best_revealed_state(
                            max_revealed_bottle_state,
                            initial_state,
                            &current_bottles,
                        );
                        improve_current_and_initial_bottles_with_revealed_state(
                            &mut current_bottles,
                            initial_state,
                            max_revealed_bottle_state,
                        );
                        latest_detected_bottles = Some(current_bottles.clone());

                        let mystery_colors = count_total_mystery_colors(max_revealed_bottle_state);
                        info!("Total mystery colors still hidden: {}", mystery_colors);
                        if mystery_colors == 0 {
                            let hidden_count = count_hidden_bottles(max_revealed_bottle_state);
                            if hidden_count > 0 {
                                info!(
                                    "All mystery colors revealed, but {} hidden bottle(s) remain locked. Switching to hidden discovery...",
                                    hidden_count
                                );

                                app_state = AppState::HiddenDiscoverBottles {
                                    initial_state: initial_state.clone(),
                                    trigger_at: now,
                                    current_moves: current_moves.clone(),
                                    known_colors: known_colors.clone(),
                                    max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                    force_hidden_discovery: false,
                                    hidden_level_retried: false,
                                    retries_remaining: BOTTLE_DETECTION_RETRIES,
                                };
                                continue;
                            }

                            info!("All mystery colors revealed! Running solver...");

                            app_state = AppState::ExecutePlanSolverMoves {
                                current_bottles: current_bottles.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                initial_state: initial_state.clone(),
                                known_colors: known_colors.clone(),
                            };
                        } else {
                            // Find best move to reveal more colors
                            let best_move = find_best_discovery_moves(
                                &current_bottles,
                                max_revealed_bottle_state,
                            );
                            #[cfg(feature = "discovery-debugging")]
                            {
                                let buffer = frame_to_window_buffer(&frame_display)?;
                                window.update_with_buffer(&buffer, width, height)?;

                                info!("Press enter to continue discovery...");
                                std::io::stdin().read_line(&mut String::new()).unwrap();
                            }

                            match best_move {
                                discovery::DiscoverResult::MoveToDiscover(best_moves) => {
                                    info!(
                                        "Best discovery move sequence found: {}",
                                        best_moves
                                            .iter()
                                            .map(|m| m.to_string())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    );
                                    app_state = AppState::MysteryExecuteDiscoverMove {
                                        moves_to_execute: best_moves,
                                        initial_state: initial_state.clone(),
                                        max_revealed_bottle_state: max_revealed_bottle_state
                                            .clone(),
                                        known_colors: known_colors.clone(),
                                        current_moves: current_moves.clone(),
                                        mystery_level_retried: *mystery_level_retried,
                                        trigger_at: now,
                                        retries_remaining: BOTTLE_DETECTION_RETRIES,
                                    };
                                }
                                discovery::DiscoverResult::NoMove => {
                                    if *mystery_level_retried {
                                        info!(
                                            "No discovery move found on the retried level either. Switching to hidden discovery..."
                                        );

                                        app_state = AppState::HiddenDiscoverBottles {
                                            trigger_at: now,
                                            known_colors: known_colors.clone(),
                                            initial_state: initial_state.clone(),
                                            max_revealed_bottle_state: max_revealed_bottle_state
                                                .clone(),
                                            current_moves: vec![],
                                            force_hidden_discovery: true,
                                            hidden_level_retried: false,
                                            retries_remaining: BOTTLE_DETECTION_RETRIES,
                                        };
                                    } else {
                                        log::debug!(
                                            "No discovery move found that reveals new colors. Retrying level..."
                                        );

                                        capture.click_at_position(RETRY_BUTTON_POS)?;

                                        app_state = AppState::MysteryDiscoverColors {
                                            trigger_at: Instant::now(),
                                            known_colors: known_colors.clone(),
                                            initial_state: initial_state.clone(),
                                            max_revealed_bottle_state: max_revealed_bottle_state
                                                .clone(),
                                            current_moves: vec![],
                                            mystery_level_retried: true,
                                            retries_remaining: BOTTLE_DETECTION_RETRIES,
                                        };
                                    }
                                }
                                discovery::DiscoverResult::AlreadySolved => {
                                    let hidden_count =
                                        count_hidden_bottles(max_revealed_bottle_state);
                                    if hidden_count > 0 {
                                        info!(
                                            "Mystery discovery finished, but {} hidden bottle(s) remain locked. Switching to hidden discovery...",
                                            hidden_count
                                        );

                                        app_state = AppState::HiddenDiscoverBottles {
                                            initial_state: initial_state.clone(),
                                            known_colors: known_colors.clone(),
                                            trigger_at: now,
                                            current_moves: current_moves.clone(),
                                            max_revealed_bottle_state: max_revealed_bottle_state
                                                .clone(),
                                            force_hidden_discovery: false,
                                            hidden_level_retried: false,
                                            retries_remaining: BOTTLE_DETECTION_RETRIES,
                                        };
                                    } else {
                                        info!(
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
                    max_revealed_bottle_state,
                    initial_state,
                    force_hidden_discovery,
                    hidden_level_retried,
                    known_colors,
                    retries_remaining,
                } => {
                    if now >= *trigger_at {
                        log::debug!(
                            "Max revealed {}",
                            max_revealed_bottle_state
                                .iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                        let current_bottles =
                            detect_bottles(&frame_raw, &mut frame_display, known_colors);

                        if let Err(error) = current_bottles {
                            if *retries_remaining == 0 {
                                return Err(anyhow!(
                                    "Error detecting bottles during hidden bottle move execution: {:?}",
                                    error
                                ));
                            }

                            let next_retries_remaining = *retries_remaining - 1;
                            warn!(
                                "Could not detect bottles during hidden bottle move execution: {:?}. Retrying in 1 second ({} retries left)...",
                                error, next_retries_remaining
                            );

                            app_state = AppState::HiddenExecuteDiscoverMove {
                                trigger_at: now + BOTTLE_DETECTION_RETRY_DELAY,
                                moves_to_execute: moves_to_execute.clone(),
                                current_moves: current_moves.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                known_colors: known_colors.clone(),
                                initial_state: initial_state.clone(),
                                force_hidden_discovery: *force_hidden_discovery,
                                hidden_level_retried: *hidden_level_retried,
                                retries_remaining: next_retries_remaining,
                            };
                            continue;
                        }

                        let current_bottles = current_bottles.unwrap();
                        latest_detected_bottles = Some(current_bottles.clone());

                        improve_best_revealed_state(
                            max_revealed_bottle_state,
                            initial_state,
                            &current_bottles,
                        );
                        if moves_to_execute.is_empty() {
                            app_state = AppState::HiddenDiscoverBottles {
                                known_colors: known_colors.clone(),
                                initial_state: initial_state.clone(),
                                trigger_at: now,
                                current_moves: current_moves.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                force_hidden_discovery: *force_hidden_discovery,
                                hidden_level_retried: *hidden_level_retried,
                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                            };
                        } else {
                            let next_move = moves_to_execute.remove(0);

                            if !next_move.can_perform_on_bottles(&current_bottles) {
                                return Err(anyhow!(
                                    "Planned hidden-bottle move cannot be performed on the currently detected bottle state. This should not happen. Move: {}, Detected bottles: {}",
                                    next_move,
                                    current_bottles
                                        .iter()
                                        .map(|b| b.to_string())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                ));
                            }

                            info!("Performing hidden-bottle move: {}.", next_move);
                            #[cfg(feature = "discovery-debugging")]
                            {
                                info!("Press enter to perform the next move...");
                                std::io::stdin().read_line(&mut String::new()).unwrap();
                            }
                            next_move.perform_move_on_device(&capture)?;

                            current_moves.push(next_move);
                            *trigger_at = Instant::now();
                        }
                    }
                }
                AppState::MysteryExecuteDiscoverMove {
                    trigger_at,
                    moves_to_execute,
                    max_revealed_bottle_state,
                    current_moves,
                    mystery_level_retried,
                    initial_state,
                    retries_remaining,
                    known_colors,
                } => {
                    if now >= *trigger_at {
                        log::debug!(
                            "Max revealed {}",
                            max_revealed_bottle_state
                                .iter()
                                .map(|b| b.to_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                        );
                        let current_bottles =
                            detect_bottles(&frame_raw, &mut frame_display, known_colors);

                        if let Err(error) = current_bottles {
                            if *retries_remaining == 0 {
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

                            let next_retries_remaining = *retries_remaining - 1;
                            warn!(
                                "Could not detect bottles during discovery move execution: {:?}. Retrying in 1 second ({} retries left)...",
                                error, next_retries_remaining
                            );

                            app_state = AppState::MysteryExecuteDiscoverMove {
                                trigger_at: now + BOTTLE_DETECTION_RETRY_DELAY,
                                moves_to_execute: moves_to_execute.clone(),
                                known_colors: known_colors.clone(),
                                initial_state: initial_state.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                current_moves: current_moves.clone(),
                                mystery_level_retried: *mystery_level_retried,
                                retries_remaining: next_retries_remaining,
                            };
                            continue;
                        }

                        let current_bottles = current_bottles.unwrap();
                        latest_detected_bottles = Some(current_bottles.clone());
                        draw_revealed_fill_markers(
                            &mut frame_display,
                            &current_bottles,
                            max_revealed_bottle_state,
                        )?;

                        if moves_to_execute.is_empty() {
                            app_state = AppState::MysteryDiscoverColors {
                                trigger_at: now,
                                known_colors: known_colors.clone(),
                                initial_state: initial_state.clone(),
                                max_revealed_bottle_state: max_revealed_bottle_state.clone(),
                                current_moves: current_moves.clone(),
                                mystery_level_retried: *mystery_level_retried,
                                retries_remaining: BOTTLE_DETECTION_RETRIES,
                            };
                        } else {
                            let next_move = moves_to_execute.remove(0);
                            if !next_move.can_perform_on_bottles(&current_bottles) {
                                return Err(anyhow!(
                                    "Planned discovery move cannot be performed on the currently detected bottle state. This should not happen. Move: {}, Detected bottles: {}",
                                    next_move,
                                    current_bottles
                                        .iter()
                                        .map(|b| b.to_string())
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                ));
                            }

                            info!("Performing discovery move: {}.", next_move);
                            debug!(
                                "Current bottles at discovery move execution: {}",
                                current_bottles
                                    .iter()
                                    .map(|b| b.to_string())
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            );
                            #[cfg(feature = "discovery-debugging")]
                            {
                                info!("Press enter to perform the next move...");
                                std::io::stdin().read_line(&mut String::new()).unwrap();
                            }
                            next_move.perform_move_on_device(&capture)?;

                            // Remove the executed move from the list
                            current_moves.push(next_move);

                            *trigger_at = Instant::now();
                        }
                    }
                }
                AppState::ExecutePlanSolverMoves {
                    initial_state,
                    max_revealed_bottle_state,
                    known_colors,
                    current_bottles,
                } => {
                    maybe_set_resolved_bottles(&mut discovery_capture, max_revealed_bottle_state);
                    finalize_discovery_capture(&mut discovery_capture);

                    let solution = solve_with_visualization(
                        max_revealed_bottle_state,
                        initial_state,
                        current_bottles,
                        &frame_raw,
                        &mut window,
                        width,
                        height,
                    )?;

                    info!("Resetting level for the solver...");
                    capture.click_at_position(RETRY_BUTTON_POS)?;
                    app_state = AppState::ExecuteFinalSolveMoves {
                        planned_moves: solution,
                        performed_moves: 0,
                        next_move_at: Instant::now(),
                        known_colors: known_colors.clone(),
                    };
                }
                AppState::ExecuteFinalSolveMoves {
                    planned_moves,
                    performed_moves,
                    next_move_at,
                    known_colors,
                } => {
                    if let Some(next) = planned_moves.get(*performed_moves).cloned() {
                        if now >= *next_move_at {
                            info!("Performing move: {}.", next);
                            match detect_bottles(&frame_raw, &mut frame_display, known_colors) {
                                Ok(current_bottles) => {
                                    let expected_state = next.get_expected_state_before_move();
                                    let state_matches =
                                        are_states_equivalent(expected_state, &current_bottles);

                                    if !state_matches {
                                        log::debug!(
                                            "Writing frame_display to discovery_move_error.png for debugging..."
                                        );
                                        let _ = imgcodecs::imwrite(
                                            "target/discovery_move_error.png",
                                            &frame_display,
                                            &Vector::new(),
                                        );
                                        log::warn!(
                                            "Expected state doesn't match the current detected state before performing a solve move. This should not happen. Move: {}, Expected state: {}, Detected state: {}",
                                            next,
                                            expected_state
                                                .iter()
                                                .map(|b| b.to_string())
                                                .collect::<Vec<_>>()
                                                .join(" "),
                                            current_bottles
                                                .iter()
                                                .map(|b| b.to_string())
                                                .collect::<Vec<_>>()
                                                .join(" ")
                                        );
                                    }

                                    latest_detected_bottles = Some(current_bottles.clone());
                                }
                                Err(error) => {
                                    warn!(
                                        "Could not detect bottles before solve move timing check: {:?}",
                                        error
                                    );
                                }
                            };

                            next.perform_move_on_device(&capture)?;
                            *performed_moves += 1;
                            *next_move_at = now;
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
                            info!("Reward screen detected, clicking 'No, thank you'...");
                            capture.click_at_position(NO_THANK_YOU_POSITIONS[0])?;
                        } else {
                            info!("No reward screen detected, proceeding to next level...");
                        }

                        app_state = AppState::ClickNextLevel {
                            trigger_at: now + NEXT_LEVEL_WAIT,
                        };
                    }
                }
            }
        }

        let overlay_snapshot = build_overlay_snapshot(&app_state, now, &motion_status_text);

        if let Some(bottles) = latest_detected_bottles.as_deref() {
            draw_detected_bottles_overlay(&mut frame_display, bottles)?;
        }

        draw_state_hud(&mut frame_display, width, &overlay_snapshot)?;

        if prev_app_state != app_state {
            if prev_app_state.get_name() != app_state.get_name() {
                if matches!(app_state, AppState::CheckForRewards { .. }) {
                    reset_color_label_assignments();
                }

                debug!(
                    "Transitioning from state {} to state {}...",
                    prev_app_state.get_name(),
                    app_state.get_name()
                );
                debug!(
                    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~"
                );
            }

            log::trace!("Clearing latest similar scores");
            recent_frame_matches.clear();

            #[cfg(feature = "save-states")]
            state_capture.capture_transition(
                &prev_app_state,
                &app_state,
                &frame_raw,
                &frame_display,
            )?;

            prev_app_state = app_state.clone();
        }

        let buffer = frame_to_window_buffer(&frame_display)?;
        window.update_with_buffer(&buffer, width, height)?;
    }

    Ok(())
}

fn are_states_equivalent(expected_state: &[Bottle], current_bottles: &[Bottle]) -> bool {
    log::trace!(
        "Current state: {}",
        current_bottles
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    log::trace!(
        "Expected state: {}",
        expected_state
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );

    current_bottles.iter().enumerate().all(|(i, b)| {
        let expected_bottle = &expected_state[i];

        if b.is_hidden_and_locked() {
            expected_bottle.hidden_requirement() == b.hidden_requirement()
        } else {
            let expected_fills = expected_bottle.get_fills();
            b.get_fills().iter().enumerate().all(|(j, c)| {
                if c == &BottleColor::Mystery {
                    true
                } else {
                    *c == expected_fills[j]
                }
            })
        }
    })
}

fn load_app_state_from_file(path: &Path) -> Result<AppState> {
    let raw_state = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read state JSON from {}", path.display()))?;

    let json: Value = serde_json::from_str(&raw_state)?;
    let app_state = json.get("app_state").ok_or_else(|| {
        anyhow!(
            "App state JSON does not contain 'app_state' field: {}",
            path.display()
        )
    })?;

    serde_json::from_value(app_state.clone()).with_context(|| {
        format!(
            "Failed to deserialize app state JSON from {}",
            path.display()
        )
    })
}

fn solve_with_visualization(
    max_revealed_bottle_state: &[Bottle],
    initial_state: &[Bottle],
    current_bottles: &[Bottle],
    frame_raw: &Mat,
    window: &mut Window,
    width: usize,
    height: usize,
) -> Result<Vec<Move>> {
    let baseline_frame = frame_raw.try_clone()?;
    let mut last_update = Instant::now() - SOLVER_VISUALIZATION_UPDATE_INTERVAL;

    log::debug!("Solver run:");
    log::debug!(
        "Initial state for solver: {}",
        initial_state
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    log::debug!(
        "Max revealed bottle state for solver: {}",
        max_revealed_bottle_state
            .iter()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut on_progress = |snapshot: water_sort_solver::SolverProgressSnapshot<'_>| {
        if !snapshot.is_goal && last_update.elapsed() < SOLVER_VISUALIZATION_UPDATE_INTERVAL {
            return;
        }
        last_update = Instant::now();

        let mut preview_frame = match baseline_frame.try_clone() {
            Ok(frame) => frame,
            Err(error) => {
                warn!("Solver visualization frame clone failed: {:?}", error);
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
            warn!("Solver visualization draw failed: {:?}", error);
            return;
        }

        match frame_to_window_buffer(&preview_frame) {
            Ok(buffer) => {
                if let Err(error) = window.update_with_buffer(&buffer, width, height) {
                    warn!("Solver visualization window update failed: {:?}", error);
                }
            }
            Err(error) => {
                warn!("Solver visualization buffer conversion failed: {:?}", error);
            }
        }

        std::thread::sleep(SOLVER_VISUALIZATION_FRAME_DELAY);
    };

    let initial_state_solver =
        build_solver_initial_bottle_state(max_revealed_bottle_state, initial_state);

    let maybe_solution = crate::solver::run_solver_with_progress(current_bottles, &mut on_progress)
        .or_else(|| crate::solver::run_solver_with_progress(&initial_state_solver, &mut on_progress));

    maybe_solution.ok_or_else(|| anyhow!("Failed to find solver solution"))
}

fn remaining_until(trigger_at: Instant, now: Instant) -> Option<Duration> {
    if trigger_at > now {
        Some(trigger_at.duration_since(now))
    } else {
        None
    }
}

fn maybe_start_discovery_capture(
    frame_raw: &Mat,
    detected_bottles: &[Bottle],
) -> Option<DiscoveryCaptureContext> {
    #[cfg(feature = "collect-test-data")]
    {
        match start_discovery_capture(frame_raw, detected_bottles) {
            Ok(capture_context) => Some(capture_context),
            Err(error) => {
                warn!("Failed to start discovery capture: {:?}", error);
                None
            }
        }
    }

    #[cfg(not(feature = "collect-test-data"))]
    {
        let _ = (frame_raw, detected_bottles);
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
            warn!(
                "Failed to persist discovery capture manifest entry: {:?}",
                error
            );
        }
    }
}

fn format_motion_status(motion_state: MotionWindowState) -> String {
    match motion_state {
        MotionWindowState::Stable => "motion: stable".to_string(),
        MotionWindowState::MovementDetected => "motion: movement detected".to_string(),
        MotionWindowState::WaitingForCoverage { missing_coverage } => {
            format!(
                "motion: building history ({:.0}ms)",
                missing_coverage.as_millis()
            )
        }
    }
}

fn build_overlay_snapshot<'a>(
    app_state: &'a AppState,
    now: Instant,
    motion_status: &str,
) -> OverlaySnapshot<'a> {
    match app_state {
        AppState::WaitingToPressStart { trigger_at } => OverlaySnapshot {
            phase: "WaitingToPressStart".to_string(),
            detail: "Preparing initial level start tap".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::ClickRetryOnNewLevel { trigger_at } => OverlaySnapshot {
            phase: "ClickRetryOnNewLevel".to_string(),
            detail: "Preparing retry tap for new level start".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::ClickNextLevel { trigger_at } => OverlaySnapshot {
            phase: "ClickNextLevel".to_string(),
            detail: "Waiting to advance to the next level".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::CheckForRewards { trigger_at } => OverlaySnapshot {
            phase: "CheckForRewards".to_string(),
            detail: "Looking for reward popup".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
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
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::AwaitPostDetectionPlan { trigger_at, .. } => OverlaySnapshot {
            phase: "AwaitPostDetectionPlan".to_string(),
            detail: "Reviewing detected bottles before planning".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::HiddenDiscoverBottles {
            trigger_at,
            current_moves,
            ..
        } => OverlaySnapshot {
            phase: "HiddenDiscoverBottles".to_string(),
            detail: "Scanning bottles to unlock hidden slots".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(0),
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::HiddenExecuteDiscoverMove {
            trigger_at,
            moves_to_execute,
            current_moves,
            ..
        } => OverlaySnapshot {
            phase: "HiddenExecuteDiscoverMove".to_string(),
            detail: "Executing hidden-slot unlock sequence".to_string(),
            until_ready: remaining_until(*trigger_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(moves_to_execute.len()),
            solve_moves: &[],
            solve_performed_moves: 0,
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
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: Some(count_total_mystery_colors(max_revealed_bottle_state)),
            discovery_total_slots: Some(max_revealed_bottle_state.len() * 4),
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(0),
            solve_moves: &[],
            solve_performed_moves: 0,
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
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: Some(count_total_mystery_colors(max_revealed_bottle_state)),
            discovery_total_slots: Some(max_revealed_bottle_state.len() * 4),
            discovery_depth: Some(current_moves.len()),
            discovery_queue: Some(moves_to_execute.len()),
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::ExecutePlanSolverMoves {
            max_revealed_bottle_state,
            ..
        } => OverlaySnapshot {
            phase: "ExecutePlanSolverMoves".to_string(),
            detail: "Planning solver moves".to_string(),
            until_ready: None,
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: Some(count_total_mystery_colors(max_revealed_bottle_state)),
            discovery_total_slots: Some(max_revealed_bottle_state.len() * 4),
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: &[],
            solve_performed_moves: 0,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: 0,
        },
        AppState::ExecuteFinalSolveMoves {
            next_move_at,
            planned_moves,
            performed_moves,
            known_colors: _,
        } => OverlaySnapshot {
            phase: "ExecuteFinalSolveMoves".to_string(),
            detail: format!(
                "Running solver move {} of {}",
                performed_moves.saturating_add(1).min(planned_moves.len()),
                planned_moves.len()
            ),
            until_ready: remaining_until(*next_move_at, now),
            motion_status: Some(motion_status.to_string()),
            discovery_hidden: None,
            discovery_total_slots: None,
            discovery_depth: None,
            discovery_queue: None,
            solve_moves: planned_moves.as_slice(),
            solve_performed_moves: *performed_moves,
            #[cfg(feature = "solver-visualization")]
            solve_current_move_index: *performed_moves,
        },
    }
}
