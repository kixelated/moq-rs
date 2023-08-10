use crate::media::{self, MapSource};
use moq_transport::AnnounceOk;
use moq_transport::{Object, VarInt};
use moq_transport_quinn::SendObjects;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

pub struct MediaRunner {
	send_objects: SendObjects,
	outgoing_ctl_sender: mpsc::Sender<moq_transport::Message>,
	outgoing_obj_sender: mpsc::Sender<moq_transport::Object>,
	incoming_ctl_receiver: broadcast::Receiver<moq_transport::Message>,
	incoming_obj_receiver: broadcast::Receiver<moq_transport::Object>,
	source: Arc<MapSource>,
}

impl MediaRunner {
	pub async fn new(
		send_objects: SendObjects,
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
			send_objects,
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

		// wait for the go ahead
		loop {
			match self.incoming_ctl_receiver.recv().await? {
				moq_transport::Message::AnnounceOk(_) => {
					break;
				}
				msg => {
					dbg!(msg);
				}
			}
		}

		// DIRTY BAD HACK
		// Subscribe to all of our own tracks immediately
		// so that the relay will ask us for them
		//
		// What we really _ought_ to do is dynamically create senders
		// for tracks only in response to received subscriptions
		for track_name in self.source.0.keys() {
			self.outgoing_ctl_sender
				.send(moq_transport::Message::Subscribe(moq_transport::Subscribe {
					track_id: moq_transport::VarInt::from(track_name.parse::<u32>()?),
					track_namespace: namespace.to_string(),
					track_name: track_name.to_string(),
				}))
				.await?;
			loop {
				match self.incoming_ctl_receiver.recv().await? {
					msg @ moq_transport::Message::Subscribe(_) => {
						dbg!(msg);
						self.outgoing_ctl_sender
							.send(moq_transport::Message::SubscribeOk(moq_transport::SubscribeOk {
								track_id: moq_transport::VarInt::from(track_name.parse::<u32>()?),
								expires: Some(std::time::Duration::from_secs(300)),
							}))
							.await?;
						break;
					}
					msg => {
						dbg!(msg);
					}
				}
			}
		}
		Ok(())
	}

	pub async fn run(&self) -> anyhow::Result<()> {
		dbg!("media_runner.run()");
		let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();

		for (track_name, track) in self.source.0.iter() {
			dbg!(&track_name);
			let mut objects = self.send_objects.clone();
			let mut track = track.clone();
			let track_name = track_name.clone();
			join_set.spawn(async move {
				loop {
					let mut segment = track.next_segment().await?;

					dbg!("segment: {:?}", &segment);
					let object = Object {
						track: VarInt::from_u32(track_name.parse::<u32>()?),
						group: segment.sequence,
						sequence: VarInt::from_u32(0), // Always zero since we send an entire group as an object
						send_order: segment.send_order,
					};
					dbg!(&object);

					let mut stream = objects.open(object).await?;

					// Write each fragment as they are available.
					while let Some(fragment) = segment.fragments.next().await {
						//dbg!(&fragment);
						stream.write_all(fragment.as_slice()).await?;
					}
				}
			});
		}

		while let Some(res) = join_set.join_next().await {
			dbg!(&res);
			//let _ = res?;
		}

		Ok(())
	}
}
