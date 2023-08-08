use tokio::sync::broadcast;

pub struct LogViewer {}

impl LogViewer {
	pub async fn new(
		incoming: (
			broadcast::Receiver<moq_transport::Message>,
			broadcast::Receiver<moq_transport::Object>,
		),
	) -> anyhow::Result<Self> {
		Ok(Self {})
	}
	pub async fn run(&self) -> anyhow::Result<()> {
		dbg!("log_viewer.run()");
		loop {
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await
		}
	}
}
