use std::{
    io::{BufRead, BufReader, Write},
    process::Command,
    thread,
    time::Duration,
};

use anyhow::{Result, anyhow};
use minifb::{MouseButton, MouseMode, Window, WindowOptions};
use opencv::{
    core::{Mat, MatTraitConst},
    videoio::{self, VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};

use crate::{
    bottles::detect_and_draw_bottles,
    capture::{frame_to_window_buffer, save_frame_png},
    constants::{START_BUTTON_POS, VIRTUAL_CAM},
    scrcpy::{click_at_position, measure_window_to_mobile_scale, start_scrcpy},
};

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

    let mut pressed_start = quick_mode;
    let mut new_level = true;
    let mut previous_right_click = false;
    let mut moves_to_perform = None;

    while window.is_open() {
        cam.read(&mut frame_raw)?;
        if frame_raw.empty() {
            continue;
        }

        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Clamp) {
            if window.get_mouse_down(MouseButton::Left) {
                println!("Clicked at: ({}, {})", x, y);
            }
        }

        let right_click = window.get_mouse_down(MouseButton::Right);
        if right_click && !previous_right_click {
            let saved_path = save_frame_png(&frame_raw)?;
            println!("Saved quick-iteration frame to {}", saved_path.display());
        }
        previous_right_click = right_click;

        if !pressed_start {
            println!("Waiting for game to start...");
            thread::sleep(Duration::from_secs(10));

            println!("Starting level...");
            click_at_position(START_BUTTON_POS);
            pressed_start = true;
            new_level = true;
        }

        let mut frame_display = frame_raw.try_clone()?;
        if new_level {
            println!("Waiting for level to load...");
            thread::sleep(Duration::from_secs(2));
            new_level = false;

            let bottles = detect_and_draw_bottles(&frame_raw, &mut frame_display);

            let buffer = frame_to_window_buffer(&frame_display)?;
            window.update_with_buffer(&buffer, width, height)?;
            if let Err(error) = bottles {
                println!("Error detecting bottles: {:?}", error);
                continue;
            }

            moves_to_perform = crate::solver::run_solver(&bottles.unwrap());
            if let Some(moves) = &moves_to_perform {
                println!("Planned moves:");
                for m in moves {
                    println!("{:?}", m);
                }
            } else {
                println!("No solution found!");
                continue;
            }
        }

        if let Some(moves) = &moves_to_perform {
            for m in moves {
                m.perform_move_on_device();
                thread::sleep(Duration::from_millis(500));
            }
            moves_to_perform = None;
        }

        //let buffer = frame_to_window_buffer(&frame_display)?;
        //window.update_with_buffer(&buffer, width, height)?;
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
