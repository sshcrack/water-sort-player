use std::time::{Duration, Instant};
use anyhow::Result;
use opencv::videoio::VideoCaptureTrait;
use water_sort_device::start_capture;

fn main() {
    if let Err(error) = measure_fps() {
        eprintln!("Error: {}", error);
        std::process::exit(1);
    }
}

fn measure_fps() -> Result<()> {
    println!("Starting FPS measurement...");
    
    let (mut cam, width, height) = start_capture(true)?;
    
    println!("Started video capture: {}x{}", width, height);
    println!("Measuring FPS for 30 seconds...\n");
    
    let mut frame = opencv::core::Mat::default();
    let start_time = Instant::now();
    let measurement_duration = Duration::from_secs(30);
    let mut frame_count = 0;
    let mut last_report = Instant::now();
    
    loop {
        if !cam.read(&mut frame)? {
            eprintln!("Failed to read frame");
            break;
        }
        
        frame_count += 1;
        let elapsed = start_time.elapsed();
        
        // Report FPS every 5 seconds
        if last_report.elapsed() >= Duration::from_secs(5) {
            let fps = frame_count as f64 / elapsed.as_secs_f64();
            println!(
                "Elapsed: {:.1}s | Frames: {} | FPS: {:.2}",
                elapsed.as_secs_f64(),
                frame_count,
                fps
            );
            last_report = Instant::now();
        }
        
        if elapsed >= measurement_duration {
            break;
        }
    }
    
    let total_duration = start_time.elapsed().as_secs_f64();
    let fps = frame_count as f64 / total_duration;
    
    println!("\n=== Final FPS Measurement ===");
    println!("Total frames: {}", frame_count);
    println!("Duration: {:.2}s", total_duration);
    println!("Average FPS: {:.2}", fps);
    
    if fps >= 15.0 {
        println!("✓ FPS requirement met (>= 15 FPS)");
        Ok(())
    } else {
        eprintln!("✗ FPS requirement NOT met (expected >= 15 FPS, got {:.2})", fps);
        Ok(())
    }
}
