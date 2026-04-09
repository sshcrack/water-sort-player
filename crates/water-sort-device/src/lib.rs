mod scrcpy;

use std::{
    io::{BufRead, BufReader, Write},
    process::Command,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
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

fn spawn_scrcpy_stdout_logger<R: BufRead + Send + 'static>(mut reader: R) -> Receiver<Result<()>> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();

    thread::spawn(move || {
        let mut line = String::new();
        let mut ready_sent = false;

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    if !ready_sent {
                        let _ = ready_tx.send(Err(anyhow!("scrcpy process ended unexpectedly")));
                    }
                    break;
                }
                Ok(_) => {
                    // Mirror scrcpy stdout so logs remain visible for the whole process lifetime.
                    print!("{line}");
                    std::io::stdout().flush().unwrap();

                    if !ready_sent && line.contains("v4l2 sink started to device:") {
                        let _ = ready_tx.send(Ok(()));
                        ready_sent = true;
                    }
                }
                Err(error) => {
                    if !ready_sent {
                        let _ = ready_tx.send(Err(error.into()));
                    }
                    break;
                }
            }
        }
    });

    ready_rx
}

pub fn wait_for_video_stream(ready_rx: Receiver<Result<()>>) -> Result<()> {
    print!("Waiting for scrcpy to initialize video stream...");

    loop {
        match ready_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(result) => {
                result?;
                println!("\nscrcpy is ready, starting video capture...");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(anyhow!("scrcpy stdout logger disconnected unexpectedly"));
            }
        }
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
    let ready_rx = spawn_scrcpy_stdout_logger(BufReader::new(child_stdout));

    wait_for_video_stream(ready_rx)?;

    thread::sleep(Duration::from_secs(2));

    let cam = VideoCapture::from_file(VIRTUAL_CAM, videoio::CAP_V4L2)?;
    let width = cam.get(videoio::CAP_PROP_FRAME_WIDTH)? as usize;
    let height = cam.get(videoio::CAP_PROP_FRAME_HEIGHT)? as usize;

    measure_window_to_mobile_scale(width, height);

    println!("Video stream dimensions: {}x{}", width, height);
    Ok((cam, width, height))
}
