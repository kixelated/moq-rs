use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time;

use tokio::io::AsyncReadExt;
use tokio::task::JoinSet; // lock across await boundaries

use moq_transport::coding::VarInt;
use moq_transport::{control, object};

use anyhow::Context;

use crate::model::{broadcast, broadcasts, segment, track};

pub struct Contribute {
	// Used to receive objects.
	// TODO split into send/receive halves.
	transport: Arc<object::Transport>,

	// Used to send control messages.
	control: control::SendShared,

	// Globally announced namespaces, which we can add ourselves to.
	broadcasts: broadcasts::Shared,

	// Active subscriptions based on the subscription ID.
	subscriptions: Arc<StdMutex<Subscriptions>>,

	// Tasks we are currently serving.
	run_broadcasts: JoinSet<anyhow::Result<()>>, // receiving subscriptions
	run_segments: JoinSet<anyhow::Result<()>>,   // receiving objects
}

impl Contribute {
	pub fn new(
		transport: Arc<object::Transport>,
		control: control::SendShared,
		broadcasts: broadcasts::Shared,
	) -> Self {
		Self {
			transport,
			control,
			broadcasts,
			subscriptions: Default::default(),
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
		let id = header.track_id.into_inner();

		let mut subscriptions = self.subscriptions.lock().unwrap();
		let track = subscriptions.get(id).context("no subscription with that ID")?;

		let segment = segment::Info {
			sequence: header.object_sequence,
			send_order: header.send_order,
			expires: Some(time::Instant::now() + time::Duration::from_secs(10)),
		};

		let segment = segment::Publisher::new(segment);
		track.segments.push(segment.subscribe());

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

		let broadcast = broadcast::Publisher::new(namespace.clone());

		// Check if this broadcast already exists globally.
		let mut broadcasts = self.broadcasts.lock().await;
		if broadcasts.contains_key(&namespace) {
			anyhow::bail!("duplicate broadcast: {}", namespace);
		}

		// Insert the subscriber into the global list of broadcasts.
		broadcasts.insert(namespace.clone(), broadcast.subscribe());

		// Create copies to send to the task.
		let broadcasts = self.broadcasts.clone();
		let subscriptions = self.subscriptions.clone();
		let control = self.control.clone();

		self.run_broadcasts.spawn(async move {
			// Serve the
			let res = Self::run_broadcast(control, subscriptions, broadcast).await;

			// Remove the broadcast on completion.
			let mut broadcasts = broadcasts.lock().await;
			broadcasts.remove(&namespace);

			res
		});

		Ok(())
	}

	fn receive_subscribe_ok(&mut self, _msg: control::SubscribeOk) -> anyhow::Result<()> {
		Ok(())
	}

	fn receive_subscribe_error(&mut self, msg: control::SubscribeError) -> anyhow::Result<()> {
		// TODO make sure we sent this subscribe
		anyhow::bail!("received SUBSCRIBE_ERROR({:?}): {}", msg.code, msg.reason)
	}

	async fn run_broadcast(
		_control: control::SendShared,
		_subscriptions: Arc<StdMutex<Subscriptions>>,
		_broadcast: broadcast::Publisher,
	) -> anyhow::Result<()> {
		/*
		// Get the next track that somebody wants to subscribe to.
		while let Some(track) = broadcast.requested().await {
			let name = track.name.clone();
			let id = subscriptions.lock().unwrap().add(track);

			control
				.send(control::Subscribe {
					track_id: VarInt::try_from(id)?,
					track_namespace: broadcast.namespace.clone(),
					track_name: name,
				})
				.await
				.context("failed to send subscription")?
		}
		*/

		Ok(())
	}
}

#[derive(Default)]
pub struct Subscriptions {
	// A lookup from subscription ID (varint) to a track being produced.
	lookup: HashMap<u64, track::Publisher>,

	// The next subscription ID
	sequence: u64,
}

impl Subscriptions {
	pub fn add(&mut self, track: track::Publisher) -> u64 {
		let id = self.sequence;
		self.sequence += 1;

		let _track_name = track.name.clone();

		self.lookup.insert(id, track);
		id
	}

	pub fn get(&mut self, id: u64) -> Option<&mut track::Publisher> {
		self.lookup.get_mut(&id)
	}

	pub fn remove(&mut self, id: u64) {
		self.lookup.remove(&id);
	}
}
