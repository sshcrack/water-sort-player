use std::{
    io::{BufReader, Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use opencv::{
    core::{CV_8UC3, Mat, Scalar},
    prelude::{MatTraitConst, MatTraitManual},
};

use crate::{
    constants::{
        SCRCPY_CONTROL_PORT, SCRCPY_DEVICE_SOCKET_NAME, SCRCPY_MAX_FPS, SCRCPY_MAX_SIZE,
        SCRCPY_SERVER_VERSION, SCRCPY_VIDEO_BIT_RATE,
    },
    position::Pos,
};

lazy_static! {
    static ref COMPUTER_TO_MOBILE_SCALE: Mutex<(f32, f32)> = Mutex::new((1.0, 1.0));
}

const SERVER_PUSH_PATH: &str = "/data/local/tmp/scrcpy-server";
const GAME_PACKAGE: &str = "com.no1ornothing.color.water.sort.woody.puzzle";

pub struct StreamDimensions {
    pub width: usize,
    pub height: usize,
}

struct FrameFeeder {
    join_handle: Option<thread::JoinHandle<Result<()>>>,
}

impl FrameFeeder {
    fn detach(&mut self) {
        let _ = self.join_handle.take();
    }
}

impl Drop for FrameFeeder {
    fn drop(&mut self) {
        self.detach();
    }
}

pub struct DirectCaptureSession {
    width: usize,
    height: usize,
    ffmpeg_child: Child,
    ffmpeg_stdout: BufReader<std::process::ChildStdout>,
    frame_feeder: FrameFeeder,
    feeder_stop: Arc<AtomicBool>,
    server_child: Child,
    frame_buffer: Vec<u8>,
}

impl DirectCaptureSession {
    pub fn dimensions(&self) -> StreamDimensions {
        StreamDimensions {
            width: self.width,
            height: self.height,
        }
    }

    pub fn read_frame_mat(&mut self, target: &mut Mat) -> Result<()> {
        self.ffmpeg_stdout.read_exact(&mut self.frame_buffer)?;

        let expected_rows = self.height as i32;
        let expected_cols = self.width as i32;
        if target.rows() != expected_rows || target.cols() != expected_cols || target.typ() != CV_8UC3 {
            *target = Mat::new_rows_cols_with_default(
                expected_rows,
                expected_cols,
                CV_8UC3,
                Scalar::default(),
            )?;
        }

        target.data_bytes_mut()?.copy_from_slice(&self.frame_buffer);

        Ok(())
    }
}

impl Drop for DirectCaptureSession {
    fn drop(&mut self) {
        self.feeder_stop.store(true, Ordering::Relaxed);

        if let Some(stdin) = self.ffmpeg_child.stdin.take() {
            drop(stdin);
        }

        if let Err(error) = self.ffmpeg_child.kill() {
            eprintln!("Failed to kill ffmpeg process: {error}");
        }
        if let Err(error) = self.server_child.kill() {
            eprintln!("Failed to kill scrcpy-server adb shell process: {error}");
        }

        if let Err(error) = Command::new(get_adb_path())
            .args([
                "forward",
                "--remove",
                &format!("tcp:{SCRCPY_CONTROL_PORT}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            eprintln!("Failed to remove adb forward: {error}");
        }

        self.frame_feeder.detach();
    }
}

pub fn get_adb_path() -> PathBuf {
    let current_executable = std::env::current_exe().unwrap();
    let current_dir = current_executable.parent().unwrap();
    let adb_path = current_dir.join("adb");

    if !adb_path.exists() {
        panic!("adb executable not found at: {}", adb_path.display());
    }

    adb_path
}

pub fn emergency_cleanup() {
    let adb_path = get_adb_path();

    let _ = Command::new(&adb_path)
        .args([
            "forward",
            "--remove",
            &format!("tcp:{SCRCPY_CONTROL_PORT}"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new(&adb_path)
        .args([
            "shell",
            "pkill",
            "-f",
            "com.genymobile.scrcpy.Server",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let parent_pid = std::process::id().to_string();
    let _ = Command::new("pkill")
        .args(["-P", &parent_pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn get_scrcpy_server_path() -> Result<PathBuf> {
    let current_executable = std::env::current_exe()?;
    let current_dir = current_executable
        .parent()
        .ok_or_else(|| anyhow!("failed to get parent directory of executable"))?;

    let server_path = current_dir.join("scrcpy-server");
    if !server_path.exists() {
        return Err(anyhow!(
            "scrcpy-server executable not found at: {}",
            server_path.display()
        ));
    }

    Ok(server_path)
}

pub fn click_at_position(pos: Pos) {
    click_at(pos.0, pos.1);
}

pub fn click_at(x: i32, y: i32) {
    let (scale_x, scale_y) = *COMPUTER_TO_MOBILE_SCALE.lock().unwrap();
    let x = (x as f32 * scale_x) as i32;
    let y = (y as f32 * scale_y) as i32;

    Command::new(get_adb_path())
        .args(["shell", "input", "tap", &x.to_string(), &y.to_string()])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
}

fn launch_game_if_needed(quick_mode: bool) -> Result<()> {
    if quick_mode {
        return Ok(());
    }

    println!("Fresh restarting game app...");
    run_adb_checked(
        &["shell", "am", "force-stop", GAME_PACKAGE],
        "force-stopping game package",
    )?;
    thread::sleep(Duration::from_millis(500));

    let output = Command::new(get_adb_path())
        .args([
            "shell",
            "monkey",
            "-p",
            GAME_PACKAGE,
            "-c",
            "android.intent.category.LAUNCHER",
            "1",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "failed to launch game package via adb shell monkey; stdout: {stdout}; stderr: {stderr}"
        ));
    }

    Ok(())
}

fn run_adb_checked(args: &[&str], description: &str) -> Result<()> {
    let output = Command::new(get_adb_path()).args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "adb command failed while {description}; stdout: {stdout}; stderr: {stderr}"
        ));
    }
    Ok(())
}

fn connect_and_read_header_with_retry(
    host_port: &str,
    timeout: Duration,
) -> Result<(TcpStream, StreamDimensions)> {
    let start = Instant::now();
    let mut last_error: String;

    loop {
        match TcpStream::connect(host_port) {
            Ok(mut stream) => match read_video_codec_header(&mut stream) {
                Ok(dimensions) => return Ok((stream, dimensions)),
                Err(error) => {
                    last_error = format!("connected but failed reading codec header: {error}");
                }
            },
            Err(error) => {
                last_error = format!("failed to connect: {error}");
            }
        }

        if start.elapsed() >= timeout {
            return Err(anyhow!(
                "failed to establish scrcpy video stream at {host_port}: {last_error}"
            ));
        }

        thread::sleep(Duration::from_millis(200));
    }
}

fn read_video_codec_header(reader: &mut impl Read) -> Result<StreamDimensions> {
    let mut header = [0u8; 12];
    reader.read_exact(&mut header)?;

    let codec = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    // h264
    if codec != 0x6832_3634 {
        return Err(anyhow!("unexpected video codec id: 0x{codec:08x}"));
    }

    let width = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let height = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;

    if width == 0 || height == 0 {
        return Err(anyhow!("invalid stream dimensions: {width}x{height}"));
    }

    Ok(StreamDimensions { width, height })
}

fn spawn_frame_feeder(
    mut socket: TcpStream,
    mut ffmpeg_stdin: std::process::ChildStdin,
    stop_requested: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        socket.set_read_timeout(Some(Duration::from_millis(500)))?;
        let mut chunk = [0u8; 64 * 1024];
        loop {
            if stop_requested.load(Ordering::Relaxed) {
                return Ok(());
            }

            let bytes = match socket.read(&mut chunk) {
                Ok(0) => return Ok(()),
                Ok(n) => n,
                Err(error)
                    if error.kind() == std::io::ErrorKind::TimedOut
                        || error.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    continue;
                }
                Err(error) => return Err(anyhow!("failed reading raw video stream: {error}")),
            };

            if bytes > 0 {
                ffmpeg_stdin.write_all(&chunk[..bytes])?;
            }
        }
    })
}

pub fn start_direct_capture(quick_mode: bool) -> Result<DirectCaptureSession> {
    launch_game_if_needed(quick_mode)?;

    let server_path = get_scrcpy_server_path()?;

    let _ = Command::new(get_adb_path())
        .args([
            "forward",
            "--remove",
            &format!("tcp:{SCRCPY_CONTROL_PORT}"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    run_adb_checked(
        &["push", server_path.to_str().unwrap(), SERVER_PUSH_PATH],
        "pushing scrcpy-server",
    )?;

    run_adb_checked(
        &[
            "forward",
            &format!("tcp:{SCRCPY_CONTROL_PORT}"),
            &format!("localabstract:{SCRCPY_DEVICE_SOCKET_NAME}"),
        ],
        "configuring adb forward",
    )?;

    let mut server_cmd = Command::new(get_adb_path());
    server_cmd
        .arg("shell")
        .arg(format!("CLASSPATH={SERVER_PUSH_PATH}"))
        .arg("app_process")
        .arg("/")
        .arg("com.genymobile.scrcpy.Server")
        .arg(SCRCPY_SERVER_VERSION)
        .arg("scid=-1")
        .arg("log_level=info")
        .arg("video=true")
        .arg("audio=false")
        .arg("control=false")
        .arg("tunnel_forward=true")
        .arg("send_dummy_byte=false")
        .arg("send_device_meta=false")
        .arg("send_codec_meta=true")
        .arg("send_frame_meta=false")
        .arg("video_codec=h264")
        .arg(format!("max_size={SCRCPY_MAX_SIZE}"))
        .arg(format!("max_fps={SCRCPY_MAX_FPS}"))
        .arg(format!("video_bit_rate={SCRCPY_VIDEO_BIT_RATE}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let server_child = server_cmd.spawn()?;

    let endpoint = format!("127.0.0.1:{SCRCPY_CONTROL_PORT}");
    let (socket, dimensions) = connect_and_read_header_with_retry(&endpoint, Duration::from_secs(12))?;

    let mut ffmpeg_cmd = Command::new("ffmpeg");
    ffmpeg_cmd
        .args([
            "-loglevel",
            "error",
            "-fflags",
            "+nobuffer+discardcorrupt",
            "-flags",
            "low_delay",
            "-codec:v",
            "h264",
            "-probesize",
            "32",
            "-analyzeduration",
            "0",
            "-f",
            "h264",
            "-i",
            "pipe:0",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "bgr24",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut ffmpeg_child = ffmpeg_cmd.spawn()?;
    let ffmpeg_stdin = ffmpeg_child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("failed to capture ffmpeg stdin"))?;
    let ffmpeg_stdout = ffmpeg_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture ffmpeg stdout"))?;

    let feeder_stop = Arc::new(AtomicBool::new(false));
    let feeder = FrameFeeder {
        join_handle: Some(spawn_frame_feeder(
            socket,
            ffmpeg_stdin,
            Arc::clone(&feeder_stop),
        )),
    };

    let frame_len = dimensions.width * dimensions.height * 3;
    let frame_buffer = vec![0u8; frame_len];

    Ok(DirectCaptureSession {
        width: dimensions.width,
        height: dimensions.height,
        ffmpeg_child,
        ffmpeg_stdout: BufReader::new(ffmpeg_stdout),
        frame_feeder: feeder,
        feeder_stop,
        server_child,
        frame_buffer,
    })
}

pub fn measure_window_to_mobile_scale(width: usize, height: usize) {
    let size = Command::new(get_adb_path())
        .args(["shell", "wm", "size"])
        .output()
        .unwrap();

    let output = String::from_utf8_lossy(&size.stdout);
    let mut mobile_width = 0.0;
    let mut mobile_height = 0.0;

    for line in output.lines() {
        if line.contains("Physical size:") {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                let dims: Vec<&str> = parts[1].trim().split('x').collect();
                if dims.len() == 2 {
                    mobile_width = dims[0].parse::<f32>().unwrap_or(0.0);
                    mobile_height = dims[1].parse::<f32>().unwrap_or(0.0);
                }
            }
        }
    }

    let scale_x = mobile_width / width as f32;
    let scale_y = mobile_height / height as f32;

    let mut scale_lock = COMPUTER_TO_MOBILE_SCALE.lock().unwrap();
    *scale_lock = (scale_x, scale_y);

    println!("Computed scale factors - X: {}, Y: {}", scale_x, scale_y);
}
