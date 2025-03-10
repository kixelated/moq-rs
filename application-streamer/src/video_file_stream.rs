use std::path::Path;
use tokio::process::ChildStdout;
use moq_karp::Dimensions;

const FFMPEG_LAUNCH_CMD: &str = "ffmpeg";

pub struct FFmpegVideoFileStream {
    ffmpeg: tokio::process::Command,
    stdout: Option<ChildStdout>,
}

impl FFmpegVideoFileStream {
    pub fn new(file_location: &str) -> Self {
        let mut ffmpeg = tokio::process::Command::new(FFMPEG_LAUNCH_CMD);
        ffmpeg
            .arg("-hide_banner")
            .arg("-v").arg("quiet")
            .arg("-stream_loop").arg("-1")
            .arg("-re")
            .arg("-i").arg(file_location)
            .arg("-c").arg("copy")
            .arg("-f").arg("mp4")
            .arg("-movflags").arg("cmaf+separate_moof+delay_moov+skip_trailer+frag_every_frame")
            .arg("pipe:1");

        Self {
            ffmpeg,
            stdout: None,
        }
    }

    pub async fn start(&mut self) {
        self.ffmpeg.stdout(std::process::Stdio::piped());
        let mut child = self.ffmpeg.spawn().expect("failed to start ffmpeg");
        self.stdout = Some(child.stdout.take().expect("child did not have a handle to stdout"));

        tokio::spawn(async move {
            child.wait().await.expect("ffmpeg process encountered an error");
        });
    }

    pub fn stop(&mut self) {
        self.stdout = None;

        // TODO: Kill the child
    }

    pub fn stdout(&mut self) -> &mut ChildStdout {
        self.stdout.as_mut().unwrap()
    }
}