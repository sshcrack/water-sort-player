use std::{
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
    app_visualization::draw_move_overlay,
    bottles::detect_and_draw_bottles,
    capture::{frame_to_window_buffer, save_frame_png},
    constants::{
        NEXT_LEVEL_BUTTON_POS, NO_THANK_YOU_REWARDS_POS, START_BUTTON_POS, VIRTUAL_CAM,
        is_color_within_tolerance,
    },
    scrcpy::{click_at_position, measure_window_to_mobile_scale, start_scrcpy},
    solver::{Move, run_solver},
};

const START_WAIT: Duration = Duration::from_secs(10);
const NEXT_LEVEL_WAIT: Duration = Duration::from_secs(5);
const NO_THANK_YOU_REWARDS_WAIT: Duration = Duration::from_secs(3);
const MOVE_DELAY: Duration = Duration::from_millis(2500);

enum AppState {
    WaitingToPressStart { trigger_at: Instant },
    ClickNextLevel { trigger_at: Instant },
    CheckForRewards { trigger_at: Instant },
    DetectAndPlan { trigger_at: Instant },
    ExecutingMoves { next_move_at: Instant },
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
    let mut planned_moves: Vec<Move> = Vec::new();
    let mut performed_moves = 0usize;

    while window.is_open() {
        cam.read(&mut frame_raw)?;
        if frame_raw.empty() {
            continue;
        }

        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp)
            && window.get_mouse_down(MouseButton::Left) {
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
        let mut active_move: Option<Move> = None;

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
                        panic!(
                            "Error: Next level button color did not match expected value. Clicking anyway..."
                        );
                    }

                    app_state = AppState::DetectAndPlan {
                        trigger_at: now + NEXT_LEVEL_WAIT,
                    };
                }
            }
            AppState::DetectAndPlan { trigger_at } => {
                if now >= *trigger_at {
                    println!("Detecting bottles for new level...");
                    planned_moves.clear();
                    performed_moves = 0;
                    let bottles = detect_and_draw_bottles(&frame_raw, &mut frame_display);
                    if let Err(error) = bottles {
                        println!("Error detecting bottles: {:?}", error);
                        app_state = AppState::ClickNextLevel {
                            trigger_at: now + NEXT_LEVEL_WAIT,
                        };
                    } else {
                        // Redraw window before running solver to show detected bottles
                        let buffer = frame_to_window_buffer(&frame_display)?;
                        window.update_with_buffer(&buffer, width, height)?;

                        println!("Running solver...");
                        if let Some(moves) = run_solver(&bottles?) {
                            println!("Planned moves:");
                            for m in &moves {
                                println!("{:?}", m);
                            }

                            planned_moves = moves;
                            performed_moves = 0;
                            if let Some(first_move) = planned_moves.first().copied() {
                                active_move = Some(first_move);
                                app_state = AppState::ExecutingMoves { next_move_at: now };
                            } else {
                                app_state = AppState::ClickNextLevel {
                                    trigger_at: now + NEXT_LEVEL_WAIT,
                                };
                            }
                        } else {
                            println!("No solution found!");
                            app_state = AppState::ClickNextLevel {
                                trigger_at: now + NEXT_LEVEL_WAIT,
                            };
                        }
                    }
                }
            }
            AppState::ExecutingMoves { next_move_at } => {
                if let Some(next) = planned_moves.get(performed_moves).copied() {
                    active_move = Some(next);
                    if now >= *next_move_at {
                        println!("Performing move: {:?}.", next);
                        next.perform_move_on_device();
                        performed_moves += 1;
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

        let mut buffer = frame_to_window_buffer(&frame_display)?;
        draw_move_overlay(
            &mut buffer,
            width,
            height,
            &planned_moves,
            performed_moves,
            active_move,
        );
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
