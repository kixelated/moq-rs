use crate::media::{self, MapSource};
use anyhow;
use log::{debug, info};
use moq_transport::{AnnounceOk, Message};
use moq_transport::{Object, VarInt};
use moq_transport_quinn::SendObjects;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

pub struct MediaRunner {
	send_objects: SendObjects,
	outgoing_ctl_sender: mpsc::Sender<moq_transport::Message>,
	incoming_ctl_receiver: broadcast::Receiver<moq_transport::Message>,
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
			incoming_ctl_receiver,
			source: Arc::new(MapSource::default()),
		})
	}
	pub async fn announce(&mut self, namespace: &str, source: Arc<media::MapSource>) -> anyhow::Result<()> {
		debug!("media_runner.announce()");
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
				_ => {}
			}
		}

		Ok(())
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		debug!("media_runner.run()");
		let source = self.source.clone();
		let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();
		let mut track_dispatcher: HashMap<String, tokio::sync::mpsc::Sender<()>> = HashMap::new();
		let mut incoming_ctl_receiver = self.incoming_ctl_receiver.resubscribe();
		let outgoing_ctl_sender = self.outgoing_ctl_sender.clone();

		// Pre-spawn tasks for each track we have
		// and let them .await on receiving the go ahead via a channel
		for (track_name, track) in source.0.iter() {
			let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
			track_dispatcher.insert(track_name.to_string(), sender);
			let mut objects = self.send_objects.clone();
			let mut track = track.clone();
			join_set.spawn(async move {
				receiver.recv().await.ok_or(anyhow::anyhow!("channel closed"))?;
				loop {
					let mut segment = track.next_segment().await?;

					debug!("segment: {:?}", &segment);
					let object = Object {
						track: VarInt::from_u32(track.name.parse::<u32>()?),
						group: segment.sequence,
						sequence: VarInt::from_u32(0), // Always zero since we send an entire group as an object
						send_order: segment.send_order,
					};
					debug!("object: {:?}", &object);

					let mut stream = objects.open(object).await?;

					// Write each fragment as they are available.
					while let Some(fragment) = segment.fragments.next().await {
						stream.write_all(fragment.as_slice()).await?;
					}
				}
			});
		}

		join_set.spawn(async move {
			loop {
				match incoming_ctl_receiver.recv().await? {
					Message::Subscribe(subscribe) => {
						debug!("Received a subscription request");

						let track_id = subscribe.track_id;
						debug!("Looking up track_id: {}", &track_id);
						// Look up track in source
						match source.0.get(&track_id.to_string()) {
							None => {
								// if track !exist, send subscribe error
								outgoing_ctl_sender.send(moq_transport::Message::SubscribeError(
									moq_transport::SubscribeError {
										track_id: subscribe.track_id,
										code: moq_transport::VarInt::from_u32(1),
										reason: "Only bad reasons (don't know what that track is)".to_string(),
									},
								));
							}
							// if track exists, send go-ahead signal to unblock task to send data to subscriber
							Some(track) => {
								debug!("We have the track! (Good news everyone)");
								track_dispatcher
									.get(&track.name)
									.ok_or(anyhow::anyhow!("missing task for track"))?
									.send(())
									.await?;
							}
						};
					}
					_ => {}
				}
			}
		});

		while let Some(res) = join_set.join_next().await {
			debug!("MediaRunner task finished with result: {:?}", &res);
			//let _ = res?;
		}

		Ok(())
	}
}
