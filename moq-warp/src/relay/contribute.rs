use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time;

use tokio::io::AsyncReadExt;
use tokio::task::JoinSet; // lock across await boundaries

use moq_transport::coding::VarInt;
use moq_transport::{control, object};

use anyhow::Context;

use super::broker;
use crate::model::{segment, track};
use crate::source::Source;

pub struct Contribute {
	// Used to receive objects.
	// TODO split into send/receive halves.
	transport: Arc<object::Transport>,

	// Used to send control messages.
	control: control::SendShared,

	// Globally announced namespaces, which we can add ourselves to.
	broker: broker::Broadcasts,

	// Active tracks being produced by this session.
	publishers: Publishers,

	// Active tracks being consumed by other sessions, used to deduplicate.
	subscribers: Subscribers,

	// Tasks we are currently serving.
	run_broadcasts: JoinSet<anyhow::Result<()>>, // receiving subscriptions
	run_segments: JoinSet<anyhow::Result<()>>,   // receiving objects
}

impl Contribute {
	pub fn new(transport: Arc<object::Transport>, control: control::SendShared, broker: broker::Broadcasts) -> Self {
		Self {
			transport,
			control,
			broker,
			publishers: Default::default(),
			subscribers: Default::default(),
			run_broadcasts: JoinSet::new(),
			run_segments: JoinSet::new(),
		}
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				res = self.run_broadcasts.join_next(), if !self.run_broadcasts.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::error!("failed to produce broadcast: {:?}", err);
					}
				},
				res = self.run_segments.join_next(), if !self.run_segments.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::error!("failed to produce segment: {:?}", err);
					}
				},
				object = self.transport.recv() => {
					let (header, stream )= object.context("failed to receive object")?;
					self.receive_object(header, stream).await?;
				},
			}
		}
	}

	pub async fn receive_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		match msg {
			control::Message::Announce(msg) => self.receive_announce(msg).await,
			control::Message::SubscribeOk(msg) => self.receive_subscribe_ok(msg),
			control::Message::SubscribeError(msg) => self.receive_subscribe_error(msg),
			// TODO make this type safe
			_ => anyhow::bail!("invalid message for contribution: {:?}", msg),
		}
	}

	async fn receive_object(&mut self, header: object::Header, stream: object::RecvStream) -> anyhow::Result<()> {
		let id = header.track_id;

		let segment = segment::Info {
			sequence: header.object_sequence,
			send_order: header.send_order,
			expires: Some(time::Instant::now() + time::Duration::from_secs(10)),
		};

		let segment = segment::Publisher::new(segment);

		self.publishers
			.push(id, segment.subscribe())
			.context("failed to publish segment")?;

		// TODO implement a timeout

		self.run_segments
			.spawn(async move { Self::run_segment(segment, stream).await });

		Ok(())
	}

	async fn run_segment(mut segment: segment::Publisher, mut stream: object::RecvStream) -> anyhow::Result<()> {
		let mut buf = [0u8; 32 * 1024];
		loop {
			let size = stream.read(&mut buf).await.context("failed to read from stream")?;
			if size == 0 {
				return Ok(());
			}

			let chunk = buf[..size].to_vec();
			segment.fragments.push(chunk.into())
		}
	}

	async fn receive_announce(&mut self, msg: control::Announce) -> anyhow::Result<()> {
		match self.receive_announce_inner(&msg).await {
			Ok(()) => {
				self.control
					.send(control::AnnounceOk {
						track_namespace: msg.track_namespace,
					})
					.await
			}
			Err(e) => {
				self.control
					.send(control::AnnounceError {
						track_namespace: msg.track_namespace,
						code: VarInt::from_u32(1),
						reason: e.to_string(),
					})
					.await
			}
		}
	}

	async fn receive_announce_inner(&mut self, msg: &control::Announce) -> anyhow::Result<()> {
		let namespace = msg.track_namespace.clone();

		let broadcast = Broadcast {
			subscribers: self.subscribers.clone(),
			publishers: self.publishers.clone(),
		};

		self.broker
			.publish(&namespace, broadcast)
			.context("failed to register broadcast")?;

		/*
		self.run_broadcasts
			.spawn(async move { Self::run_broadcast(broker, control, subscribers, publishers).await });
		*/

		Ok(())
	}

	fn receive_subscribe_ok(&mut self, _msg: control::SubscribeOk) -> anyhow::Result<()> {
		Ok(())
	}

	fn receive_subscribe_error(&mut self, msg: control::SubscribeError) -> anyhow::Result<()> {
		// TODO return the error to the subscriber
		self.publishers.remove(msg.track_id)?;

		anyhow::bail!("received SUBSCRIBE_ERROR({:?}): {}", msg.code, msg.reason)
	}

	/*
	async fn run_broadcast(
		mut broker: broker::Publisher,
		mut control: control::SendShared,
		mut subscribers: Subscribers,
		mut publishers: Publishers,
	) -> anyhow::Result<()> {
		while let Some(request) = broker.request().await {
			if let Some(track) = subscribers.get(&request.name) {
				request.respond(Ok(track));
				continue;
			}

			let name = request.name.clone();
			let track = track::Publisher::new(name.clone());

			// Reply to the request with the subscriber
			request.respond(Ok(track.subscribe()));

			// Tell all other subscribers to use this subscription
			subscribers.set(&name, track.subscribe());

			// Get a unique ID for the publisher.
			let id = publishers.insert(track);

			// TODO close the publisher if this fails
			control
				.send(control::Subscribe {
					track_id: id,
					track_namespace: broker.namespace.clone(),
					track_name: name,
				})
				.await
				.context("failed to send subscription")?;
		}

		Ok(())
	}
	*/
}

struct Broadcast {
	subscribers: Subscribers,
	publishers: Publishers,
}

impl Source for Broadcast {
	fn subscribe(&mut self, name: &str) -> Option<track::Subscriber> {
		if let Some(subscriber) = self.subscribers.get(name) {
			return Some(subscriber);
		}

		//let subscriber = self.publishers.create(name);
		let subscriber = track::Publisher::new(name).subscribe();

		// Save the subscriber for duplication.
		self.subscribers.set(name, subscriber.clone());

		// Use the subscription ourselves
		Some(subscriber)
	}
}

#[derive(Clone, Default)]
pub struct Subscribers {
	// A lookup from name to an existing subscription (new subscribers)
	lookup: Arc<Mutex<HashMap<String, track::Subscriber>>>,
}

impl Subscribers {
	// Duplicates subscriptions, returning a new subscription ID if this is the first subscription.
	pub fn get(&mut self, name: &str) -> Option<track::Subscriber> {
		self.lookup.lock().unwrap().get(name).cloned()
	}

	pub fn set(&mut self, name: &str, track: track::Subscriber) {
		let existing = self.lookup.lock().unwrap().insert(name.into(), track);
		assert!(existing.is_none(), "duplicate track name");
	}
}

#[derive(Clone, Default)]
pub struct Publishers {
	this: Arc<Mutex<PublishersInner>>,
}

#[derive(Default)]
struct PublishersInner {
	// A list of tracks that we need to populate.
	//pending: Vec<track::Publisher>,

	// A lookup from subscription ID to a track being produced (new publishers)
	lookup: HashMap<VarInt, track::Publisher>,

	// The next subscription ID
	next: u64,
}

impl Publishers {
	/*
	pub fn create(&mut self, track: track::Publisher) -> (VarInt) {
		let mut next = self.next.lock().unwrap();
		let id = VarInt::try_from(*next).unwrap();
		*next += 1;

		self.lookup.lock().unwrap().insert(id, track);
		id
	}
	*/

	pub fn push(&mut self, id: VarInt, segment: segment::Subscriber) -> anyhow::Result<()> {
		let mut this = self.this.lock().unwrap();

		let publisher = this.lookup.get_mut(&id).context("no track with that ID")?;
		publisher.segments.push(segment);
		Ok(())
	}

	pub fn remove(&mut self, id: VarInt) -> anyhow::Result<()> {
		let mut this = self.this.lock().unwrap();
		this.lookup.remove(&id).context("no track with that ID")?;
		Ok(())
	}
}
