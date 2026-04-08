mod scrcpy;
mod server;
mod video_decoder;

use std::{
    thread,
    time::Duration,
};

use anyhow::anyhow;
use opencv::videoio::VideoCapture;
pub use scrcpy::*;
use server::ScrcpyServer;
use video_decoder::{H264StreamReader, open_video_capture};

pub fn start_capture(quick_mode: bool) -> anyhow::Result<(VideoCapture, usize, usize)> {
    println!("Starting scrcpy server...");
    
    let mut server = ScrcpyServer::start(quick_mode)?;
    let video_socket = server.take_video_socket()
        .ok_or_else(|| anyhow!("Failed to get video socket"))?;
    
    println!("Initializing H.264 stream reader...");
    let stream_reader = H264StreamReader::new(video_socket)?;
    let (width, height) = stream_reader.get_dimensions();
    
    println!("Opening video capture from stream...");
    // Give the writer thread time to start
    thread::sleep(Duration::from_millis(500));
    
    let cam = open_video_capture(stream_reader.get_fifo_path())?;
    
    // Keep stream_reader alive by leaking it (it will clean up on process exit)
    // This is necessary because the FIFO writer thread needs to keep running
    std::mem::forget(stream_reader);
    std::mem::forget(server);
    
    measure_window_to_mobile_scale(width as usize, height as usize);
    
    println!("Video stream dimensions: {}x{}", width, height);
    Ok((cam, width as usize, height as usize))
}
