use tokio::process::Child;
use crate::xvfb::Xvfb;

pub struct XvfbUser {
    xvfb_display: u32,
    start_cmd: String,
    child: Option<Child>,
}

impl XvfbUser {
    pub fn new(xvfb: &Xvfb, start_cmd: &str) -> Self {
        Self { xvfb_display: xvfb.display(), start_cmd: start_cmd.to_string(), child: None }
    }

    pub fn start(&mut self) {
        set_display_var(self.xvfb_display);

        let start_cmd = self.start_cmd.clone();

        tokio::spawn(async move {
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(start_cmd)
                .spawn()
                .expect("failed to start xvfb user");
        });

        // tracing::info!("Xvfb user started on display :{} with command:{}", self.xvfb_display, self.start_cmd);
    }

    pub async fn stop(&mut self) {
        self.child
            .take()
            .expect("no child process to stop")
            .kill()
            .await
            .expect("failed to kill child process");
    }
}

fn set_display_var(display: u32) {
    unsafe {
        std::env::set_var("DISPLAY", format!(":{}", display));
    }
    assert_eq!(std::env::var("DISPLAY").unwrap(), format!(":{}", display), "failed to set DISPLAY variable");
}