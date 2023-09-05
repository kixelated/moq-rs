use crate::media::{self, MapSource};
use anyhow::bail;
use log::{debug, error};
use moq_transport::message::Message;
use moq_transport::message::{Announce, SubscribeError, SubscribeOk};
use moq_transport::{object, Object, VarInt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use webtransport_generic::Session as WTSession;

pub struct MediaRunner<S: WTSession> {
	send_objects: object::Sender<S>,
	outgoing_ctl_sender: mpsc::Sender<Message>,
	incoming_ctl_receiver: broadcast::Receiver<Message>,
	source: Arc<MapSource>,
}

impl<S: WTSession> MediaRunner<S> {
	pub async fn new(
		send_objects: object::Sender<S>,
		outgoing: mpsc::Sender<Message>,
		incoming: (broadcast::Receiver<Message>, broadcast::Receiver<Object>),
	) -> anyhow::Result<Self> {
		let outgoing_ctl_sender = outgoing;
		let (incoming_ctl_receiver, _incoming_obj_receiver) = incoming;
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
			.send(Message::Announce(Announce {
				track_namespace: namespace.to_string(),
			}))
			.await?;

		// wait for the go ahead
		loop {
			match self.incoming_ctl_receiver.recv().await? {
				Message::AnnounceOk(_) => {
					break;
				}
				Message::AnnounceError(announce_error) => {
					error!(
						"Failed to announce namespace '{}' with error code '{}' and reason '{}'",
						&namespace, &announce_error.code, &announce_error.reason
					);
					// TODO: Think about how to recover here? Retry?
					bail!("Failed to announce namespace");
				}
				_ => {
					// TODO: work out how to ignore unknown/unrelated messages here without consuming them prematurely
				}
			}
		}

		Ok(())
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		debug!("media_runner.run()");
		let source = self.source.clone();
		let mut join_set: JoinSet<anyhow::Result<()>> = tokio::task::JoinSet::new();
		let mut track_dispatcher: HashMap<String, tokio::sync::mpsc::Sender<VarInt>> = HashMap::new();
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
				let track_id = receiver.recv().await.ok_or(anyhow::anyhow!("channel closed"))?;
				// TODO: validate track_id is valid (not already in use), for now just trust subscribers are correct
				loop {
					let mut segment = track.next_segment().await?;

					debug!("segment: {:?}", &segment);
					let object = Object {
						track: track_id,
						group: segment.sequence,
						sequence: VarInt::from_u32(0), // Always zero since we send an entire group as an object
						send_order: segment.send_order,
					};
					debug!("object: {:?}", &object);

					let mut stream = objects.open(object).await?;

					// Write each fragment as they are available.
					while let Some(fragment) = segment.fragments.next().await {
						stream.write_all(&fragment).await?;
					}
				}
			});
		}

		join_set.spawn(async move {
			loop {
				if let Message::Subscribe(subscribe) = incoming_ctl_receiver.recv().await? {
					debug!("Received a subscription request");

					let track_id = subscribe.track_id;
					let track_name = subscribe.track_name;
					debug!("Looking up track_name: {} (track_id: {})", &track_name, &track_id);
					// Look up track in source
					match source.0.get(&track_name.to_string()) {
						None => {
							// if track !exist, send subscribe error
							outgoing_ctl_sender
								.send(Message::SubscribeError(SubscribeError {
									track_id: subscribe.track_id,
									code: moq_transport::VarInt::from_u32(1),
									reason: "Only bad reasons (don't know what that track is)".to_string(),
								}))
								.await?;
						}
						// if track exists, send go-ahead signal to unblock task to send data to subscriber
						Some(track) => {
							debug!("We have the track! (Good news everyone)");
							track_dispatcher
								.get(&track.name)
								.ok_or(anyhow::anyhow!("missing task for track"))?
								.send(track_id)
								.await?;
							outgoing_ctl_sender
								.send(Message::SubscribeOk(SubscribeOk {
									track_id: subscribe.track_id,
									expires: Some(VarInt::from_u32(0)), // valid until unsubscribed
								}))
								.await?;
						}
					};
				}
			}
		});

		while let Some(res) = join_set.join_next().await {
			debug!("MediaRunner task finished with result: {:?}", &res);
		}

		Ok(())
	}
}
