use log::{debug, info};
use tokio::{select, sync::broadcast};

pub struct LogViewer {
	incoming_ctl_receiver: broadcast::Receiver<moq_transport::Message>,
	incoming_obj_receiver: broadcast::Receiver<moq_transport::Object>,
}

impl LogViewer {
	pub async fn new(
		incoming: (
			broadcast::Receiver<moq_transport::Message>,
			broadcast::Receiver<moq_transport::Object>,
		),
	) -> anyhow::Result<Self> {
		Ok(Self {
			incoming_ctl_receiver: incoming.0,
			incoming_obj_receiver: incoming.1,
		})
	}
	pub async fn run(&mut self) -> anyhow::Result<()> {
		debug!("log_viewer.run()");

		loop {
			select! {
			msg = self.incoming_ctl_receiver.recv() => {
				info!(
				"Received incoming MOQT Control message: {:?}",
				&msg?
			);}
			obj = self.incoming_obj_receiver.recv() => {
				info!(
				"Received incoming MOQT Object with header: {:?}",
				&obj?
			);}
			}
		}
	}
}
