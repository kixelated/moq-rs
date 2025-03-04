use tokio::process::Child;
use moq_karp::Dimensions;

pub struct Xvfb {
    resolution: Dimensions,
    display: u32,
    color_depth: u32,

    xvfb_process: Option<Child>,
}

impl Xvfb {
    pub fn new(resolution: Dimensions, display: u32) -> Self {
        Self {
            resolution,
            display,
            color_depth: 24,
            xvfb_process: None,
        }
    }

    pub fn resolution(&self) -> Dimensions {
        self.resolution
    }
    pub fn display(&self) -> u32 {
        self.display
    }

    pub async fn start(&mut self) {
        self.xvfb_process = Some(tokio::process::Command::new("Xvfb")
            .arg(format!(":{}", self.display))
            .arg("-screen")
            .arg("0")
            .arg(format!("{}x{}x{}", self.resolution.width, self.resolution.height, self.color_depth))
            .spawn()
            .expect("failed to start Xvfb"));

        match self.xvfb_process {
            Some(ref mut process) => {
                process.wait().await.expect("Xvfb process encountered an error while starting");
            }
            _ => {}
        }
    }

    pub async fn stop(&mut self) {
        match self.xvfb_process {
            Some(ref mut process) => {
                process.kill().await.expect("failed to kill Xvfb");
                self.xvfb_process = None;
            },
            None => panic!("Xvfb is not running"),
        }
    }
}