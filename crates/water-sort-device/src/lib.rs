mod scrcpy;

use std::{
    io::{BufRead, BufReader, Write},
    process::Command,
    thread,
    time::Duration,
};

use anyhow::{Result, anyhow};
use opencv::videoio::{self, VideoCapture, VideoCaptureTraitConst};
pub use scrcpy::*;

pub(crate) const VIRTUAL_CAM: &str = "/dev/video10";

pub fn load_loopback_device() {
    Command::new("sudo")
        .args(["modprobe", "v4l2loopback", "video_nr=10"])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

pub fn wait_for_video_stream<R: BufRead>(mut reader: R) -> Result<()> {
    let mut line = String::new();
    print!("Waiting for scrcpy to initialize video stream...");

    loop {
        let bytes_read = reader.read_line(&mut line)?;
        print!(".");
        std::io::stdout().flush().unwrap();

        if bytes_read == 0 {
            return Err(anyhow::anyhow!("scrcpy process ended unexpectedly"));
        }

        if line.contains("v4l2 sink started to device:") {
            println!("\nscrcpy is ready, starting video capture...");
            break;
        }

        line.clear();
    }

    Ok(())
}

pub fn start_capture(quick_mode: bool) -> anyhow::Result<(VideoCapture, usize, usize)> {
    println!("Loading loopback video device...");
    load_loopback_device();

    let mut scrcpy = start_scrcpy(quick_mode)?;
    println!("scrcpy started successfully.");

    let child_stdout = scrcpy
        .take_stdout()
        .ok_or_else(|| anyhow!("failed to capture scrcpy stdout"))?;

    wait_for_video_stream(BufReader::new(child_stdout))?;

    thread::sleep(Duration::from_secs(2));

    let cam = VideoCapture::from_file(VIRTUAL_CAM, videoio::CAP_V4L2)?;
    let width = cam.get(videoio::CAP_PROP_FRAME_WIDTH)? as usize;
    let height = cam.get(videoio::CAP_PROP_FRAME_HEIGHT)? as usize;

    measure_window_to_mobile_scale(width, height);

    println!("Video stream dimensions: {}x{}", width, height);
    Ok((cam, width, height))
}
