use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use anyhow::{Result, anyhow};
use opencv::{
    prelude::*,
    videoio::{self, VideoCapture},
};

use crate::server::{read_u32_be, read_u64_be};

const SC_CODEC_ID_H264: u32 = 0x68323634; // "h264" in ASCII
const SC_PACKET_FLAG_CONFIG: u64 = 1u64 << 63;
const SC_PACKET_FLAG_KEY_FRAME: u64 = 1u64 << 62;

pub struct H264StreamReader {
    _socket: Arc<Mutex<TcpStream>>,
    fifo_path: PathBuf,
    fifo_writer: Arc<Mutex<Option<File>>>,
    stop_flag: Arc<AtomicBool>,
    width: u32,
    height: u32,
    writer_thread: Option<thread::JoinHandle<()>>,
}

impl H264StreamReader {
    pub fn new(mut socket: TcpStream) -> Result<Self> {
        // Read codec ID (4 bytes)
        let codec_id = read_u32_be(&mut socket)?;
        println!("Received codec ID: 0x{:08x}", codec_id);
        
        if codec_id == 0 {
            return Err(anyhow!("Video stream explicitly disabled by device"));
        }
        
        if codec_id == 1 {
            return Err(anyhow!("Stream configuration error on device"));
        }
        
        if codec_id != SC_CODEC_ID_H264 {
            return Err(anyhow!("Unsupported codec: 0x{:08x}", codec_id));
        }
        
        // Read video dimensions (8 bytes: width + height)
        let width = read_u32_be(&mut socket)?;
        let height = read_u32_be(&mut socket)?;
        println!("Video dimensions: {}x{}", width, height);
        
        if width == 0 || height == 0 {
            return Err(anyhow!("Invalid video dimensions: {}x{}", width, height));
        }

        // Create FIFO for streaming H.264 data
        let fifo_path = PathBuf::from(format!("/tmp/scrcpy_stream_{}.h264", std::process::id()));
        
        // Remove if exists
        let _ = std::fs::remove_file(&fifo_path);
        
        // Create FIFO using libc
        let fifo_path_cstr = std::ffi::CString::new(fifo_path.to_str().unwrap())?;
        unsafe {
            if libc::mkfifo(fifo_path_cstr.as_ptr(), 0o644) != 0 {
                return Err(anyhow!("Failed to create FIFO"));
            }
        }
        
        println!("Created FIFO at: {}", fifo_path.display());

        let socket = Arc::new(Mutex::new(socket));
        let fifo_writer = Arc::new(Mutex::new(None));
        let stop_flag = Arc::new(AtomicBool::new(false));

        // Start writer thread
        let writer_thread = {
            let socket = Arc::clone(&socket);
            let fifo_writer = Arc::clone(&fifo_writer);
            let fifo_path = fifo_path.clone();
            let stop_flag = Arc::clone(&stop_flag);
            
            thread::spawn(move || {
                if let Err(e) = write_h264_stream(socket, fifo_writer, fifo_path, stop_flag) {
                    eprintln!("H264 writer thread error: {}", e);
                }
            })
        };

        Ok(H264StreamReader {
            _socket: socket,
            fifo_path,
            fifo_writer,
            stop_flag,
            width,
            height,
            writer_thread: Some(writer_thread),
        })
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn get_fifo_path(&self) -> &PathBuf {
        &self.fifo_path
    }
}

impl Drop for H264StreamReader {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        
        // Close FIFO writer
        {
            let mut writer_guard = self.fifo_writer.lock().unwrap();
            *writer_guard = None;
        }
        
        // Wait for writer thread
        if let Some(thread) = self.writer_thread.take() {
            let _ = thread.join();
        }
        
        // Remove FIFO
        let _ = std::fs::remove_file(&self.fifo_path);
    }
}

fn write_h264_stream(
    socket: Arc<Mutex<TcpStream>>,
    fifo_writer: Arc<Mutex<Option<File>>>,
    fifo_path: PathBuf,
    stop_flag: Arc<AtomicBool>,
) -> Result<()> {
    let mut config_packet: Option<Vec<u8>> = None;
    
    loop {
        if stop_flag.load(Ordering::SeqCst) {
            break;
        }
        
        // Read packet header (12 bytes)
        let pts_flags = {
            let mut socket = socket.lock().unwrap();
            read_u64_be(&mut *socket)?
        };
        let packet_size = {
            let mut socket = socket.lock().unwrap();
            read_u32_be(&mut *socket)?
        };
        
        if packet_size == 0 || packet_size > 10_000_000 {
            return Err(anyhow!("Invalid packet size: {}", packet_size));
        }
        
        // Read packet data
        let mut packet_data = vec![0u8; packet_size as usize];
        {
            let mut socket = socket.lock().unwrap();
            socket.read_exact(&mut packet_data)?;
        }
        
        let is_config = (pts_flags & SC_PACKET_FLAG_CONFIG) != 0;
        let _is_key_frame = (pts_flags & SC_PACKET_FLAG_KEY_FRAME) != 0;
        
        // Store config packet for later
        if is_config {
            config_packet = Some(packet_data.clone());
            continue;
        }
        
        // Write to FIFO
        let mut writer_guard = fifo_writer.lock().unwrap();
        if let Some(ref mut writer) = *writer_guard {
            // On first non-config packet, write config first if we have it
            if let Some(ref config) = config_packet {
                writer.write_all(config)?;
                config_packet = None; // Only write once
            }
            
            writer.write_all(&packet_data)?;
            writer.flush()?;
        } else {
            // Try to open FIFO
            match OpenOptions::new().write(true).open(&fifo_path) {
                Ok(file) => {
                    println!("FIFO opened for writing in thread");
                    *writer_guard = Some(file);
                    // Try again next iteration
                }
                Err(_) => {
                    // Reader hasn't opened yet, wait a bit
                    drop(writer_guard);
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
    
    Ok(())
}

pub fn open_video_capture(fifo_path: &PathBuf) -> Result<VideoCapture> {
    println!("Opening video capture from FIFO: {}", fifo_path.display());
    
    // Open with CAP_FFMPEG backend explicitly
    let cam = VideoCapture::from_file(
        fifo_path.to_str().unwrap(),
        videoio::CAP_FFMPEG,
    )?;
    
    if !cam.is_opened()? {
        return Err(anyhow!("Failed to open video capture from FIFO"));
    }
    
    println!("Video capture opened successfully");
    Ok(cam)
}
