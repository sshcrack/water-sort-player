use std::{sync::atomic::Ordering, thread, time::{Duration, Instant}};

use anyhow::anyhow;
use minifb::{Window, WindowOptions};
use opencv::core::Mat;

use crate::{capture::frame_to_window_buffer, scrcpy::{emergency_cleanup, start_direct_capture}, shutdown::{SHUTDOWN_REQUESTED, install_signal_handler}};

const FPS_MEASURE_TARGET: u32 = 15;
const FPS_MEASURE_FRAME_TIME: Duration = Duration::from_millis(1000 / FPS_MEASURE_TARGET as u64);

pub fn run(_quick_mode: bool) -> anyhow::Result<()> {
    install_signal_handler();
    SHUTDOWN_REQUESTED.store(false, Ordering::Relaxed);

    println!(
        "Feature 'fps-measure' enabled: gameplay automation disabled, measuring stream FPS only."
    );
    println!(
        "Target FPS: {} (frame budget: {} ms)",
        FPS_MEASURE_TARGET,
        FPS_MEASURE_FRAME_TIME.as_millis()
    );
    println!("Starting direct scrcpy-server capture...");
    let mut capture = start_direct_capture(false)?;

    thread::sleep(Duration::from_secs(2));

    let dimensions = capture.dimensions();
    let width = dimensions.width;
    let height = dimensions.height;

    println!("Video stream dimensions: {}x{}", width, height);
    let mut window = Window::new("FPS Measure", width, height, WindowOptions::default())?;
    let mut frame_raw = Mat::default();

    let mut measure_window_start = Instant::now();
    let mut measured_frames: u32 = 0;

    while window.is_open() {
        if SHUTDOWN_REQUESTED.load(Ordering::Relaxed) {
            println!("Shutdown requested, exiting FPS measure loop...");
            break;
        }

        let loop_start = Instant::now();

        if let Err(error) = capture.read_frame_mat(&mut frame_raw) {
            emergency_cleanup();
            return Err(anyhow!(
                "Failed to read frame from direct capture stream: {error:?}"
            ));
        }

        let buffer = frame_to_window_buffer(&frame_raw)?;
        window.update_with_buffer(&buffer, width, height)?;

        measured_frames += 1;
        let window_elapsed = measure_window_start.elapsed();
        if window_elapsed >= Duration::from_secs(1) {
            let measured_fps = measured_frames as f64 / window_elapsed.as_secs_f64();
            println!(
                "Measured FPS: {:.2} (target: {})",
                measured_fps, FPS_MEASURE_TARGET
            );
            measured_frames = 0;
            measure_window_start = Instant::now();
        }

        let frame_elapsed = loop_start.elapsed();
        if frame_elapsed < FPS_MEASURE_FRAME_TIME {
            thread::sleep(FPS_MEASURE_FRAME_TIME - frame_elapsed);
        }
    }

    Ok(())
}