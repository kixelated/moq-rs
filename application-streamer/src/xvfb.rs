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

    pub fn start(&mut self) {
        let display = self.display.clone();
        let resolution = self.resolution.clone();
        let color_depth = self.color_depth.clone();

        tokio::spawn(async move {
            tokio::process::Command::new("Xvfb")
                .arg(format!(":{}", display))
                .arg("-screen")
                .arg("0")
                .arg(format!("{}x{}x{}", resolution.width, resolution.height, color_depth))
                .spawn()
                .expect("failed to start Xvfb");
        });

        // tracing::info!("Xvfb started on display :{}", self.display);
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