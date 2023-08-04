use anyhow::Context;

use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet; // allows locking across await

use moq_transport::message::{Announce, AnnounceError, AnnounceOk, Subscribe, SubscribeError, SubscribeOk};
use moq_transport::{object, VarInt};
use webtransport_generic::Session as WTSession;

use crate::model::{segment, track};
use crate::relay::{
	message::{Component, Distribute},
	Broker, BrokerUpdate,
};

pub struct Session<S: WTSession> {
	// Objects are sent to the client
	objects: object::Sender<S>,

	// Used to send and receive control messages.
	control: Component<Distribute>,

	// Globally announced namespaces, which can be subscribed to.
	broker: Broker,

	// A list of tasks that are currently running.
	run_subscribes: JoinSet<SubscribeError>, // run subscriptions, sending the returned error if they fail
}

impl<S: WTSession> Session<S> {
	pub fn new(objects: object::Sender<S>, control: Component<Distribute>, broker: Broker) -> Self {
		Self {
			objects,
			control,
			broker,
			run_subscribes: JoinSet::new(),
		}
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		// Announce all available tracks and get a stream of updates.
		let (available, mut updates) = self.broker.available();
		for namespace in available {
			self.on_available(BrokerUpdate::Insert(namespace)).await?;
		}

		loop {
			tokio::select! {
				res = self.run_subscribes.join_next(), if !self.run_subscribes.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					self.control.send(res).await?;
				},
				delta = updates.next() => {
					let delta = delta.expect("no more broadcasts");
					self.on_available(delta).await?;
				},
				msg = self.control.recv() => {
					let msg = msg.context("failed to receive control message")?;
					self.receive_message(msg).await?;
				},
			}
		}
	}

	async fn receive_message(&mut self, msg: Distribute) -> anyhow::Result<()> {
		match msg {
			Distribute::AnnounceOk(msg) => self.receive_announce_ok(msg),
			Distribute::AnnounceError(msg) => self.receive_announce_error(msg),
			Distribute::Subscribe(msg) => self.receive_subscribe(msg).await,
		}
	}

	fn receive_announce_ok(&mut self, _msg: AnnounceOk) -> anyhow::Result<()> {
		// TODO make sure we sent this announce
		Ok(())
	}

	fn receive_announce_error(&mut self, msg: AnnounceError) -> anyhow::Result<()> {
		// TODO make sure we sent this announce
		// TODO remove this from the list of subscribable broadcasts.
		log::warn!("received error {:?}", msg);
		Ok(())
	}

	async fn receive_subscribe(&mut self, msg: Subscribe) -> anyhow::Result<()> {
		match self.receive_subscribe_inner(&msg).await {
			Ok(()) => {
				self.control
					.send(SubscribeOk {
						track_id: msg.track_id,
						expires: None,
					})
					.await
			}
			Err(e) => {
				self.control
					.send(SubscribeError {
						track_id: msg.track_id,
						code: VarInt::from_u32(1),
						reason: e.to_string(),
					})
					.await
			}
		}
	}

	async fn receive_subscribe_inner(&mut self, msg: &Subscribe) -> anyhow::Result<()> {
		let track = self
			.broker
			.subscribe(&msg.track_namespace, &msg.track_name)
			.context("could not find broadcast")?;

		// TODO can we just clone self?
		let objects = self.objects.clone();
		let track_id = msg.track_id;

		self.run_subscribes
			.spawn(async move { Self::run_subscribe(objects, track_id, track).await });

		Ok(())
	}

	async fn run_subscribe(
		objects: object::Sender<S>,
		track_id: VarInt,
		mut track: track::Subscriber,
	) -> SubscribeError {
		let mut tasks = JoinSet::new();
		let mut result = None;

		loop {
			tokio::select! {
				// Accept new segments added to the track.
				segment = track.next_segment(), if result.is_none() => {
					match segment {
						Ok(segment) => {
							let objects = objects.clone();
							tasks.spawn(async move { Self::serve_group(objects, track_id, segment).await });
						},
						Err(e) => {
							result = Some(SubscribeError {
								track_id,
								code: e.code,
								reason: e.reason,
							})
						},
					}
				},
				// Poll any pending segments until they exit.
				res = tasks.join_next(), if !tasks.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::error!("failed to serve segment: {:?}", err);
					}
				},
				else => return result.unwrap()
			}
		}
	}

	async fn serve_group(
		mut objects: object::Sender<S>,
		track_id: VarInt,
		mut segment: segment::Subscriber,
	) -> anyhow::Result<()> {
		let object = object::Header {
			track: track_id,
			group: segment.sequence,
			sequence: VarInt::from_u32(0), // Always zero since we send an entire group as an object
			send_order: segment.send_order,
		};

		let mut stream = objects.open(object).await?;

		// Write each fragment as they are available.
		while let Some(fragment) = segment.fragments.next().await {
			stream.write_all(&fragment).await?;
		}

		// NOTE: stream is automatically closed when dropped

		Ok(())
	}

	async fn on_available(&mut self, delta: BrokerUpdate) -> anyhow::Result<()> {
		match delta {
			BrokerUpdate::Insert(name) => {
				self.control
					.send(Announce {
						track_namespace: name.clone(),
					})
					.await
			}
			BrokerUpdate::Remove(name, error) => {
				self.control
					.send(AnnounceError {
						track_namespace: name,
						code: error.code,
						reason: error.reason,
					})
					.await
			}
		}
	}
}
