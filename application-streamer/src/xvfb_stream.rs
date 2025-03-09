use tokio::process::ChildStdout;
use crate::xvfb::Xvfb;

const FFMPEG_LAUNCH_CMD: &str = "ffmpeg";

pub struct FFmpegXvfbStream {
    ffmpeg: tokio::process::Command,
    stdout: Option<ChildStdout>,
}

impl FFmpegXvfbStream {
    pub fn new(fps: u32, xvfb: &Xvfb) -> Self {
        let mut ffmpeg = tokio::process::Command::new(FFMPEG_LAUNCH_CMD);
        ffmpeg
            .arg("-y")
            .arg("-r").arg(fps.to_string())
            .arg("-f").arg("x11grab")
            .arg("-s").arg(format!("{}x{}", xvfb.resolution().width, xvfb.resolution().height))
            .arg("-i").arg(format!(":{}", xvfb.display()))
            .arg("-preset").arg("superfast")
            .arg("-f").arg("mp4")
            .arg("pipe:1");

        Self {
            ffmpeg,
            stdout: None,
        }
    }

    pub fn start(&mut self) {
        self.ffmpeg.stdout(std::process::Stdio::piped());
        let mut child = self.ffmpeg.spawn().expect("failed to start ffmpeg");
        self.stdout = Some(child.stdout.take().expect("child did not have a handle to stdout"));

        tokio::spawn(async move {
            child.wait().await.expect("ffmpeg process encountered an error");
        });

        // tracing::info!("FFmpeg started");
    }

    pub fn stop(&mut self) {
        self.stdout = None;

        // TODO: Kill the child
    }

    pub fn stdout(&mut self) -> &mut ChildStdout {
        self.stdout.as_mut().unwrap()
    }
}