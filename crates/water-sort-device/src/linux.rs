use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{
        Mutex,
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use opencv::{
    core::{Mat, MatTraitConst},
    videoio::{self, VideoCapture, VideoCaptureTrait, VideoCaptureTraitConst},
};

use crate::CaptureDeviceBackend;
const VIRTUAL_CAM: &str = "/dev/video10";
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

pub struct ScrcpyChild(Child);

impl ScrcpyChild {
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.0.stdout.take()
    }
}

impl Drop for ScrcpyChild {
    fn drop(&mut self) {
        if let Err(error) = self.0.kill() {
            eprintln!("Failed to kill scrcpy process: {}", error);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrcpyConfig {
    pub max_size: u16,
    pub max_fps: u16,
    pub video_bit_rate: String,
    pub video_codec: String,
    pub launch_app: String,
}

impl Default for ScrcpyConfig {
    fn default() -> Self {
        Self {
            max_size: 800,
            max_fps: 15,
            video_bit_rate: "2M".to_string(),
            video_codec: "h264".to_string(),
            launch_app: "com.no1ornothing.color.water.sort.woody.puzzle".to_string(),
        }
    }
}

pub struct ScrcpyVirtualCamBackend {
    child: Option<ScrcpyChild>,
    cam: Option<VideoCapture>,
    scale: Mutex<(f32, f32)>,
    config: ScrcpyConfig,
}

impl Default for ScrcpyVirtualCamBackend {
    fn default() -> Self {
        Self {
            child: None,
            cam: None,
            scale: Mutex::new((1.0, 1.0)),
            config: ScrcpyConfig::default(),
        }
    }
}

impl ScrcpyVirtualCamBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.as_mut().and_then(ScrcpyChild::take_stdout)
    }

    fn start_scrcpy(&mut self, quick_mode: bool) -> Result<()> {
        let current_executable = std::env::current_exe()?;
        let current_dir = current_executable
            .parent()
            .ok_or_else(|| anyhow!("failed to get parent directory of executable"))?;

        let scrcpy_path = current_dir.join("scrcpy");
        if !scrcpy_path.exists() {
            return Err(anyhow!(
                "scrcpy executable not found at: {}",
                scrcpy_path.display()
            ));
        }

        let mut cmd = Command::new("stdbuf");
        cmd.arg("-oL")
            .arg(
                scrcpy_path
                    .to_str()
                    .ok_or_else(|| anyhow!("scrcpy executable path is not valid UTF-8"))?,
            )
            .arg("--stay-awake")
            .arg("--no-audio")
            .arg("--mouse=disabled")
            .arg("--keyboard=disabled")
            .arg("--gamepad=disabled")
            .arg(format!("--max-size={}", self.config.max_size))
            .arg(format!("--max-fps={}", self.config.max_fps))
            .arg(format!("--video-bit-rate={}", self.config.video_bit_rate))
            .arg(format!("--video-codec={}", self.config.video_codec))
            .arg("--no-clipboard-autosync")
            .arg("--window-title=WaterSortPlayer")
            .arg("--no-video-playback")
            .arg(format!("--v4l2-sink={}", VIRTUAL_CAM))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if !quick_mode {
            cmd.arg(format!("--start-app=+{}", self.config.launch_app));
        }

        let child = cmd.spawn().context("failed to spawn scrcpy")?;
        self.child = Some(ScrcpyChild(child));
        Ok(())
    }
}

impl CaptureDeviceBackend for ScrcpyVirtualCamBackend {
    fn start_capture(&mut self, quick_mode: bool) -> Result<(usize, usize)> {
        println!("Loading loopback video device...");
        load_loopback_device();

        self.start_scrcpy(quick_mode)?;
        println!("scrcpy started successfully.");

        let child_stdout = self
            .take_stdout()
            .ok_or_else(|| anyhow!("failed to capture scrcpy stdout"))?;
        let ready_rx = spawn_scrcpy_stdout_logger(BufReader::new(child_stdout));

        wait_for_video_stream(ready_rx)?;

        thread::sleep(Duration::from_secs(2));

        let cam = VideoCapture::from_file(VIRTUAL_CAM, videoio::CAP_V4L2)?;
        let width = cam.get(videoio::CAP_PROP_FRAME_WIDTH)? as usize;
        let height = cam.get(videoio::CAP_PROP_FRAME_HEIGHT)? as usize;

        let (scale_x, scale_y) = measure_window_to_mobile_scale(width, height)?;
        {
            let mut scale_lock = self.scale.lock().unwrap();
            *scale_lock = (scale_x, scale_y);
        }

        println!("Video stream dimensions: {}x{}", width, height);
        self.cam = Some(cam);
        Ok((width, height))
    }

    fn capture_frame(&mut self) -> Result<Mat> {
        let cam = self
            .cam
            .as_mut()
            .ok_or_else(|| anyhow!("capture has not been started yet"))?;

        let mut frame = Mat::default();
        cam.read(&mut frame)
            .context("failed to read frame from video capture")?;

        if frame.empty() {
            return Err(anyhow!("received empty frame from video capture"));
        }

        Ok(frame)
    }

    fn get_scale(&self) -> (f32, f32) {
        *self.scale.lock().unwrap()
    }
}
