use std::{
    io::Read,
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, anyhow, Context};

const SC_DEVICE_SERVER_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const SCRCPY_VERSION: &str = "3.1.1";

pub struct ScrcpyServer {
    server_process: Child,
    video_socket: Option<TcpStream>,
    local_port: u16,
    stop_flag: Arc<AtomicBool>,
}

impl ScrcpyServer {
    pub fn start(quick_mode: bool) -> Result<Self> {
        // Get paths
        let current_executable = std::env::current_exe()?;
        let current_dir = current_executable
            .parent()
            .ok_or_else(|| anyhow!("failed to get parent directory of executable"))?;
        
        let server_path = current_dir.join("scrcpy-server");
        if !server_path.exists() {
            return Err(anyhow!(
                "scrcpy-server not found at: {}",
                server_path.display()
            ));
        }

        let adb_path = get_adb_path()?;

        // Push server to device
        println!("Pushing scrcpy-server to device...");
        push_server(&adb_path, &server_path)?;

        // Find an available port
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let local_port = listener.local_addr()?.port();
        drop(listener); // Release the port for adb to use

        // Set up adb forward
        println!("Setting up adb forward on port {}...", local_port);
        setup_adb_forward(&adb_path, local_port)?;

        // Start server process
        println!("Starting scrcpy server...");
        let mut server_process = start_server_process(&adb_path, quick_mode)?;

        // Wait a bit for server to initialize
        thread::sleep(Duration::from_millis(500));

        // Check if process is still running
        match server_process.try_wait()? {
            Some(status) => {
                return Err(anyhow!("Server process exited prematurely with status: {}", status));
            }
            None => {}
        }

        // Connect to the forwarded port
        println!("Connecting to server...");
        let video_socket = connect_to_server(local_port)?;

        let stop_flag = Arc::new(AtomicBool::new(false));

        Ok(ScrcpyServer {
            server_process,
            video_socket: Some(video_socket),
            local_port,
            stop_flag,
        })
    }

    pub fn take_video_socket(&mut self) -> Option<TcpStream> {
        self.video_socket.take()
    }

    pub fn get_local_port(&self) -> u16 {
        self.local_port
    }
}

impl Drop for ScrcpyServer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        
        // Close socket first
        drop(self.video_socket.take());

        // Kill server process
        if let Err(e) = self.server_process.kill() {
            eprintln!("Failed to kill scrcpy server process: {}", e);
        }

        // Remove adb forward
        if let Ok(adb_path) = get_adb_path() {
            let _ = Command::new(adb_path)
                .args(["forward", "--remove", &format!("tcp:{}", self.local_port)])
                .output();
        }
    }
}

fn get_adb_path() -> Result<PathBuf> {
    let current_executable = std::env::current_exe()?;
    let current_dir = current_executable.parent().unwrap();
    let adb_path = current_dir.join("adb");

    if !adb_path.exists() {
        return Err(anyhow!("adb executable not found at: {}", adb_path.display()));
    }

    Ok(adb_path)
}

fn push_server(adb_path: &PathBuf, server_path: &PathBuf) -> Result<()> {
    let output = Command::new(adb_path)
        .args(["push", server_path.to_str().unwrap(), SC_DEVICE_SERVER_PATH])
        .output()
        .context("Failed to push scrcpy-server to device")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to push server: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn setup_adb_forward(adb_path: &PathBuf, local_port: u16) -> Result<()> {
    let device_socket = format!("localabstract:scrcpy_{:08x}", 0x12345678u32);
    
    let output = Command::new(adb_path)
        .args([
            "forward",
            &format!("tcp:{}", local_port),
            &device_socket,
        ])
        .output()
        .context("Failed to set up adb forward")?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to set up adb forward: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn start_server_process(adb_path: &PathBuf, quick_mode: bool) -> Result<Child> {
    let _device_socket = format!("scrcpy_{:08x}", 0x12345678u32);
    
    let mut cmd = Command::new(adb_path);
    cmd.args([
        "shell",
        &format!("CLASSPATH={}", SC_DEVICE_SERVER_PATH),
        "app_process",
        "/",
        "com.genymobile.scrcpy.Server",
        SCRCPY_VERSION,
    ]);
    
    // Add parameters
    cmd.arg(format!("scid={:08x}", 0x12345678u32));
    cmd.arg("log_level=info");
    cmd.arg("video_bit_rate=2000000");
    cmd.arg("max_size=800");
    cmd.arg("max_fps=15");
    cmd.arg("video_codec=h264");
    cmd.arg("audio=false");
    cmd.arg("control=false");
    cmd.arg("cleanup=true");
    cmd.arg("power_off_on_close=false");
    cmd.arg("clipboard_autosync=false");
    cmd.arg("downsize_on_error=false");
    cmd.arg("stay_awake=true");

    if !quick_mode {
        cmd.arg("start_app=com.no1ornothing.color.water.sort.woody.puzzle");
    }
    
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd.spawn().context("Failed to start server process")?;

    Ok(child)
}

fn connect_to_server(local_port: u16) -> Result<TcpStream> {
    let max_attempts = 50;
    let retry_delay = Duration::from_millis(100);

    for attempt in 0..max_attempts {
        match TcpStream::connect(format!("127.0.0.1:{}", local_port)) {
            Ok(mut stream) => {
                // Try to read one byte to ensure connection is ready
                stream.set_read_timeout(Some(Duration::from_millis(500)))?;
                stream.set_nodelay(true)?;
                
                let mut buf = [0u8; 1];
                match stream.read_exact(&mut buf) {
                    Ok(_) => {
                        // Success! Server sent the dummy byte
                        println!("Connected to scrcpy server");
                        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                        return Ok(stream);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || 
                              e.kind() == std::io::ErrorKind::TimedOut => {
                        // Server not ready yet, retry
                        if attempt < max_attempts - 1 {
                            thread::sleep(retry_delay);
                            continue;
                        }
                    }
                    Err(e) => {
                        return Err(anyhow!("Failed to read from server: {}", e));
                    }
                }
            }
            Err(e) => {
                if attempt < max_attempts - 1 {
                    thread::sleep(retry_delay);
                    continue;
                } else {
                    return Err(anyhow!(
                        "Failed to connect to server after {} attempts: {}",
                        max_attempts,
                        e
                    ));
                }
            }
        }
    }

    Err(anyhow!("Failed to connect to server"))
}

pub fn read_u32_be(socket: &mut TcpStream) -> Result<u32> {
    let mut buf = [0u8; 4];
    socket.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

pub fn read_u64_be(socket: &mut TcpStream) -> Result<u64> {
    let mut buf = [0u8; 8];
    socket.read_exact(&mut buf)?;
    Ok(u64::from_be_bytes(buf))
}
