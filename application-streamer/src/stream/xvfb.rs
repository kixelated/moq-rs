use application_streamer::Xvfb;
use crate::{FFmpegStream};

pub fn new(fps: u32, xvfb: &Xvfb) -> FFmpegStream {
    FFmpegStream::new(vec![
        "-y",
        "-r", &fps.to_string(),
        "-f", "x11grab",
        "-s", &format!("{}x{}", xvfb.resolution().width, xvfb.resolution().height),
        "-i", &format!(":{}", xvfb.display()),
        "-preset", "superfast",
        "-f", "mp4",
        "pipe:1"
    ])
}