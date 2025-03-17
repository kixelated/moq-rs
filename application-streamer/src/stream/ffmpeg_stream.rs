use tokio::process::{Child, ChildStdout};

const FFMPEG_LAUNCH_CMD: &str = "ffmpeg";

pub struct FFmpegStream {
    ffmpeg: tokio::process::Command,
    child: Option<Child>,
}

impl FFmpegStream {
    pub fn new(args: Vec<&str>) -> Self {
        let mut ffmpeg = tokio::process::Command::new(FFMPEG_LAUNCH_CMD);
        for arg in args {
            ffmpeg.arg(arg);
            println!("{}", arg);
        }

        Self {
            ffmpeg,
            child: None,
        }
    }

    pub fn start(&mut self) {
        self.ffmpeg.stdout(std::process::Stdio::piped());
        self.child = Some(self.ffmpeg.spawn().expect("failed to start ffmpeg"));
    }

    pub async fn stop(&mut self) {
        if let Some(child) = &mut self.child {
            child.kill().await.expect("failed to kill ffmpeg");
        }
    }

    pub fn stdout(&mut self) -> ChildStdout {
        match &mut self.child {
            Some(child) => child.stdout.take().expect("child did not have a handle to stdout"),
            None => panic!("FFmpeg not started"),
        }
    }
}