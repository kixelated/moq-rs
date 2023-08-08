use crate::media::{self, MapSource};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::mpsc;

pub struct MediaRunner {
	outgoing_ctl_sender: mpsc::Sender<moq_transport::Message>,
	outgoing_obj_sender: mpsc::Sender<moq_transport::Object>,
	incoming_ctl_receiver: broadcast::Receiver<moq_transport::Message>,
	incoming_obj_receiver: broadcast::Receiver<moq_transport::Object>,
	source: Arc<MapSource>,
}

impl MediaRunner {
	pub async fn new(
		outgoing: (
			mpsc::Sender<moq_transport::Message>,
			mpsc::Sender<moq_transport::Object>,
		),
		incoming: (
			broadcast::Receiver<moq_transport::Message>,
			broadcast::Receiver<moq_transport::Object>,
		),
	) -> anyhow::Result<Self> {
		let (outgoing_ctl_sender, outgoing_obj_sender) = outgoing;
		let (incoming_ctl_receiver, incoming_obj_receiver) = incoming;
		Ok(Self {
			outgoing_ctl_sender,
			outgoing_obj_sender,
			incoming_ctl_receiver,
			incoming_obj_receiver,
			source: Arc::new(MapSource::default()),
		})
	}
	pub async fn announce(&mut self, namespace: &str, source: Arc<media::MapSource>) -> anyhow::Result<()> {
		dbg!("media_runner.announce()");
		// Only allow one souce at a time for now?
		self.source = source;

		// ANNOUNCE the namespace
		self.outgoing_ctl_sender
			.send(moq_transport::Message::Announce(moq_transport::Announce {
				track_namespace: namespace.to_string(),
			}))
			.await?;
		Ok(())
		// TODO: wait for AnnounceOk to come back
	}

	pub async fn run(&self) -> anyhow::Result<()> {
		dbg!("media_runner.run()");
		loop {
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await
		}
		//		todo!()
	}
}
