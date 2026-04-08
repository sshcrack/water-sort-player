use std::{
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
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

const VIDEO_CODEC_H264: u32 = 0x6832_3634;
const FRAME_HEADER_LEN: usize = 12;
const MAX_PACKET_SIZE: usize = 2 * 1024 * 1024;
const FEEDER_POLL_TIMEOUT: Duration = Duration::from_millis(300);
const FRAME_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const USE_FRAMED_STREAM: bool = false;

#[derive(Debug, Clone, Copy)]
struct FramedVideoPacketHeader {
    pts_and_flags: u64,
    payload_len: usize,
}

impl FramedVideoPacketHeader {
    fn parse(raw_header: [u8; FRAME_HEADER_LEN]) -> Result<Self> {
        let pts_and_flags = u64::from_be_bytes([
            raw_header[0],
            raw_header[1],
            raw_header[2],
            raw_header[3],
            raw_header[4],
            raw_header[5],
            raw_header[6],
            raw_header[7],
        ]);
        let payload_len =
            u32::from_be_bytes([raw_header[8], raw_header[9], raw_header[10], raw_header[11]])
                as usize;

        if payload_len > MAX_PACKET_SIZE {
            return Err(anyhow!(
                "invalid scrcpy packet size: {payload_len} > {MAX_PACKET_SIZE}"
            ));
        }

        Ok(Self {
            pts_and_flags,
            payload_len,
        })
    }

    fn is_config_packet(&self) -> bool {
        (self.pts_and_flags >> 63) & 1 == 1
    }
}

struct WorkerThread {
    join_handle: Option<thread::JoinHandle<Result<()>>>,
}

impl WorkerThread {
    fn join_and_log(&mut self, context: &str) {
        let Some(join_handle) = self.join_handle.take() else {
            return;
        };

        match join_handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                eprintln!("{context} exited with error: {error}");
            }
            Err(_) => {
                eprintln!("{context} panicked");
            }
        }
    }
}

impl Drop for WorkerThread {
    fn drop(&mut self) {
        let _ = self.join_handle.take();
    }
}

pub struct DirectCaptureSession {
    width: usize,
    height: usize,
    ffmpeg_child: Child,
    packet_feeder: WorkerThread,
    decoder_input_writer: WorkerThread,
    decoded_frame_reader: WorkerThread,
    feeder_stop: Arc<AtomicBool>,
    server_child: Child,
    frame_rx: Receiver<Vec<u8>>,
    feeder_error: Arc<Mutex<Option<String>>>,
    has_received_first_frame: bool,
}

impl DirectCaptureSession {
    pub fn dimensions(&self) -> StreamDimensions {
        StreamDimensions {
            width: self.width,
            height: self.height,
        }
    }

    pub fn read_frame_mat(&mut self, target: &mut Mat) -> Result<()> {
        let frame_buffer = loop {
            if let Some(error) = self.take_worker_error() {
                return Err(anyhow!("capture worker failed: {error}"));
            }

            if let Some(status) = self.ffmpeg_child.try_wait()? {
                return Err(anyhow!(
                    "ffmpeg process exited unexpectedly with status: {status}"
                ));
            }

            match self.frame_rx.recv_timeout(FRAME_WAIT_TIMEOUT) {
                Ok(buffer) => break buffer,
                Err(RecvTimeoutError::Timeout) => {
                    if self.has_received_first_frame {
                        // Keep previous frame if decode is temporarily stalled.
                        return Ok(());
                    }

                    // Before the first frame, keep waiting while workers remain healthy.
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(anyhow!(
                        "decoded frame channel disconnected; capture pipeline stopped"
                    ));
                }
            }
        };

        let expected_rows = self.height as i32;
        let expected_cols = self.width as i32;
        if target.rows() != expected_rows
            || target.cols() != expected_cols
            || target.typ() != CV_8UC3
        {
            *target = Mat::new_rows_cols_with_default(
                expected_rows,
                expected_cols,
                CV_8UC3,
                Scalar::default(),
            )?;
        }

        target.data_bytes_mut()?.copy_from_slice(&frame_buffer);
        self.has_received_first_frame = true;

        Ok(())
    }

    fn take_worker_error(&self) -> Option<String> {
        self.feeder_error
            .lock()
            .ok()
            .and_then(|mut lock| lock.take())
    }
}

impl Drop for DirectCaptureSession {
    fn drop(&mut self) {
        self.feeder_stop.store(true, Ordering::Relaxed);

        if let Err(error) = self.ffmpeg_child.kill() {
            eprintln!("Failed to kill ffmpeg process: {error}");
        }
        let _ = self.ffmpeg_child.wait();

        if let Err(error) = self.server_child.kill() {
            eprintln!("Failed to kill scrcpy-server adb shell process: {error}");
        }
        let _ = self.server_child.wait();

        if let Err(error) = Command::new(get_adb_path())
            .args(["forward", "--remove", &format!("tcp:{SCRCPY_CONTROL_PORT}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            eprintln!("Failed to remove adb forward: {error}");
        }

        self.packet_feeder.join_and_log("Packet feeder");
        self.decoder_input_writer
            .join_and_log("Decoder input writer");
        self.decoded_frame_reader
            .join_and_log("Decoded frame reader");
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
        .args(["forward", "--remove", &format!("tcp:{SCRCPY_CONTROL_PORT}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let _ = Command::new(&adb_path)
        .args(["shell", "pkill", "-f", "com.genymobile.scrcpy.Server"])
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
    if codec != VIDEO_CODEC_H264 {
        return Err(anyhow!("unexpected video codec id: 0x{codec:08x}"));
    }

    let width = u32::from_be_bytes([header[4], header[5], header[6], header[7]]) as usize;
    let height = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;

    if width == 0 || height == 0 {
        return Err(anyhow!("invalid stream dimensions: {width}x{height}"));
    }

    Ok(StreamDimensions { width, height })
}

fn read_exact_interruptible(
    reader: &mut impl Read,
    buffer: &mut [u8],
    stop_requested: &AtomicBool,
) -> Result<()> {
    let mut offset = 0usize;
    while offset < buffer.len() {
        if stop_requested.load(Ordering::Relaxed) {
            return Err(anyhow!("capture stop requested"));
        }

        match reader.read(&mut buffer[offset..]) {
            Ok(0) => {
                return Err(anyhow!(
                    "unexpected eof while reading {} bytes (received {})",
                    buffer.len(),
                    offset
                ));
            }
            Ok(n) => {
                offset += n;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::TimedOut
                    || error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::Interrupted =>
            {
                continue;
            }
            Err(error) => {
                return Err(anyhow!("socket read failed: {error}"));
            }
        }
    }

    Ok(())
}

fn read_framed_video_packet(
    reader: &mut impl Read,
    payload: &mut Vec<u8>,
    stop_requested: &AtomicBool,
) -> Result<FramedVideoPacketHeader> {
    let mut raw_header = [0u8; FRAME_HEADER_LEN];
    read_exact_interruptible(reader, &mut raw_header, stop_requested)?;

    let header = FramedVideoPacketHeader::parse(raw_header)?;
    if header.payload_len == 0 {
        return Err(anyhow!("received empty framed packet payload"));
    }

    payload.clear();
    payload.resize(header.payload_len, 0);
    read_exact_interruptible(reader, payload, stop_requested)?;

    Ok(header)
}

fn report_worker_error(error_slot: &Arc<Mutex<Option<String>>>, message: String) {
    if let Ok(mut lock) = error_slot.lock() {
        if lock.is_none() {
            *lock = Some(message);
        }
    }
}

fn spawn_packet_feeder(
    mut socket: TcpStream,
    packet_tx: SyncSender<Vec<u8>>,
    stop_requested: Arc<AtomicBool>,
    feeder_error: Arc<Mutex<Option<String>>>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        socket.set_read_timeout(Some(FEEDER_POLL_TIMEOUT))?;

        if !USE_FRAMED_STREAM {
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
                            || error.kind() == std::io::ErrorKind::WouldBlock
                            || error.kind() == std::io::ErrorKind::Interrupted =>
                    {
                        continue;
                    }
                    Err(error) => {
                        let message = format!("failed reading raw video stream: {error}");
                        report_worker_error(&feeder_error, message.clone());
                        return Err(anyhow!(message));
                    }
                };

                let packet = chunk[..bytes].to_vec();
                if let Err(error) = packet_tx.send(packet) {
                    let message = format!("packet channel send failed: {error}");
                    report_worker_error(&feeder_error, message.clone());
                    return Err(anyhow!(message));
                }
            }
        }

        let mut packet_payload = Vec::new();
        let mut pending_config = Vec::new();

        loop {
            if stop_requested.load(Ordering::Relaxed) {
                return Ok(());
            }

            let header =
                match read_framed_video_packet(&mut socket, &mut packet_payload, &stop_requested) {
                    Ok(header) => header,
                    Err(error) => {
                        if stop_requested.load(Ordering::Relaxed) {
                            return Ok(());
                        }

                        let message = format!("failed reading framed scrcpy packet: {error}");
                        report_worker_error(&feeder_error, message.clone());
                        return Err(anyhow!(message));
                    }
                };

            let packet_to_send = if header.is_config_packet() {
                pending_config.extend_from_slice(&packet_payload);
                continue;
            } else if pending_config.is_empty() {
                packet_payload.clone()
            } else {
                let mut merged = Vec::with_capacity(pending_config.len() + packet_payload.len());
                merged.extend_from_slice(&pending_config);
                merged.extend_from_slice(&packet_payload);
                pending_config.clear();
                merged
            };

            if let Err(error) = packet_tx.send(packet_to_send) {
                let message = format!("packet channel send failed: {error}");
                report_worker_error(&feeder_error, message.clone());
                return Err(anyhow!(message));
            }
        }
    })
}

fn spawn_decoder_input_writer(
    mut ffmpeg_stdin: ChildStdin,
    packet_rx: Receiver<Vec<u8>>,
    stop_requested: Arc<AtomicBool>,
    feeder_error: Arc<Mutex<Option<String>>>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        loop {
            if stop_requested.load(Ordering::Relaxed) {
                return Ok(());
            }

            let packet = match packet_rx.recv_timeout(FEEDER_POLL_TIMEOUT) {
                Ok(packet) => packet,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            };

            if let Err(error) = ffmpeg_stdin.write_all(&packet) {
                let message = format!("failed writing packet to ffmpeg stdin: {error}");
                report_worker_error(&feeder_error, message.clone());
                return Err(anyhow!(message));
            }
        }
    })
}

fn spawn_decoded_frame_reader(
    mut ffmpeg_stdout: ChildStdout,
    frame_len: usize,
    frame_tx: SyncSender<Vec<u8>>,
    stop_requested: Arc<AtomicBool>,
    feeder_error: Arc<Mutex<Option<String>>>,
) -> thread::JoinHandle<Result<()>> {
    thread::spawn(move || {
        let mut frame_buffer = vec![0u8; frame_len];

        loop {
            if stop_requested.load(Ordering::Relaxed) {
                return Ok(());
            }

            if let Err(error) = ffmpeg_stdout.read_exact(&mut frame_buffer) {
                if stop_requested.load(Ordering::Relaxed) {
                    return Ok(());
                }

                let message = format!("failed reading decoded frame from ffmpeg stdout: {error}");
                report_worker_error(&feeder_error, message.clone());
                return Err(anyhow!(message));
            }

            match frame_tx.try_send(frame_buffer.clone()) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    // Keep latency low by dropping frames when consumer lags behind.
                }
                Err(TrySendError::Disconnected(_)) => return Ok(()),
            }
        }
    })
}

pub fn start_direct_capture(quick_mode: bool) -> Result<DirectCaptureSession> {
    launch_game_if_needed(quick_mode)?;

    let server_path = get_scrcpy_server_path()?;

    let _ = Command::new(get_adb_path())
        .args(["forward", "--remove", &format!("tcp:{SCRCPY_CONTROL_PORT}")])
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
        .arg(format!("send_frame_meta={USE_FRAMED_STREAM}"))
        .arg("video_codec=h264")
        .arg(format!("max_size={SCRCPY_MAX_SIZE}"))
        .arg(format!("max_fps={SCRCPY_MAX_FPS}"))
        .arg(format!("video_bit_rate={SCRCPY_VIDEO_BIT_RATE}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let server_child = server_cmd.spawn()?;

    let endpoint = format!("127.0.0.1:{SCRCPY_CONTROL_PORT}");
    let (socket, dimensions) =
        connect_and_read_header_with_retry(&endpoint, Duration::from_secs(12))?;

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
    let feeder_error = Arc::new(Mutex::new(None));

    let (packet_tx, packet_rx) = mpsc::sync_channel::<Vec<u8>>(64);
    let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(3);

    let packet_feeder = WorkerThread {
        join_handle: Some(spawn_packet_feeder(
            socket,
            packet_tx,
            Arc::clone(&feeder_stop),
            Arc::clone(&feeder_error),
        )),
    };

    let decoder_input_writer = WorkerThread {
        join_handle: Some(spawn_decoder_input_writer(
            ffmpeg_stdin,
            packet_rx,
            Arc::clone(&feeder_stop),
            Arc::clone(&feeder_error),
        )),
    };

    let frame_len = dimensions.width * dimensions.height * 3;
    let decoded_frame_reader = WorkerThread {
        join_handle: Some(spawn_decoded_frame_reader(
            ffmpeg_stdout,
            frame_len,
            frame_tx,
            Arc::clone(&feeder_stop),
            Arc::clone(&feeder_error),
        )),
    };

    Ok(DirectCaptureSession {
        width: dimensions.width,
        height: dimensions.height,
        ffmpeg_child,
        packet_feeder,
        decoder_input_writer,
        decoded_frame_reader,
        feeder_stop,
        server_child,
        frame_rx,
        feeder_error,
        has_received_first_frame: false,
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

#[cfg(test)]
mod tests {
    use super::{
        FRAME_HEADER_LEN, FramedVideoPacketHeader, MAX_PACKET_SIZE, read_framed_video_packet,
    };
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn parses_framed_header() {
        let pts_and_flags = 0x4000_0000_0000_1234u64;
        let payload_len = 5u32;

        let mut raw = [0u8; FRAME_HEADER_LEN];
        raw[..8].copy_from_slice(&pts_and_flags.to_be_bytes());
        raw[8..].copy_from_slice(&payload_len.to_be_bytes());

        let parsed = FramedVideoPacketHeader::parse(raw).expect("header should parse");
        assert_eq!(parsed.pts_and_flags, pts_and_flags);
        assert_eq!(parsed.payload_len, payload_len as usize);
        assert!(!parsed.is_config_packet());
    }

    #[test]
    fn rejects_oversized_framed_header() {
        let mut raw = [0u8; FRAME_HEADER_LEN];
        raw[8..].copy_from_slice(&((MAX_PACKET_SIZE as u32) + 1).to_be_bytes());

        let result = FramedVideoPacketHeader::parse(raw);
        assert!(result.is_err());
    }

    #[test]
    fn reads_framed_packet_payload() {
        let payload = [1u8, 2, 3, 4];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x8000_0000_0000_0000u64.to_be_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&payload);

        let mut cursor = Cursor::new(bytes);
        let mut out = Vec::new();
        let stop = AtomicBool::new(false);
        let header =
            read_framed_video_packet(&mut cursor, &mut out, &stop).expect("packet should read");

        assert!(header.is_config_packet());
        assert_eq!(header.payload_len, payload.len());
        assert_eq!(out, payload);
    }
}
