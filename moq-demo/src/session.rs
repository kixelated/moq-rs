use anyhow::Context;

use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use std::sync::Arc;

use moq_transport::coding::VarInt;
use moq_transport::{control, object, server, setup};
use moq_warp::{broadcasts, Broadcast, Segment, Track};

pub struct Session {
	// Used to send/receive data streams.
	transport: Arc<object::Transport>,

	// Used to send/receive control messages.
	control: control::Stream,

	// The list of available broadcasts for the session.
	broadcasts: broadcasts::Shared,

	// Active tasks being run.
	tasks: JoinSet<anyhow::Result<()>>,
}

impl Session {
	pub async fn accept(session: server::Accept, broadcasts: broadcasts::Shared) -> anyhow::Result<Session> {
		// Accep the WebTransport session.
		// OPTIONAL validate the conn.uri() otherwise call conn.reject()
		let session = session
			.accept()
			.await
			.context("failed to accept WebTransport session")?;

		session
			.setup()
			.versions
			.iter()
			.find(|v| **v == setup::Version::DRAFT_00)
			.context("failed to find supported version")?;

		match session.setup().role {
			setup::Role::Subscriber => {}
			_ => anyhow::bail!("TODO publishing not yet supported"),
		}

		let setup = setup::Server {
			version: setup::Version::DRAFT_00,
			role: setup::Role::Publisher,
		};

		let (transport, control) = session.accept(setup).await?;

		let session = Self {
			transport: Arc::new(transport),
			control,
			broadcasts: broadcasts.clone(),
			tasks: JoinSet::new(),
		};

		Ok(session)
	}

	pub async fn serve(mut self) -> anyhow::Result<()> {
		let mut broadcasts = self.broadcasts.lock().unwrap().updates();

		loop {
			tokio::select! {
				msg = self.control.recv() => {
					let msg = msg.context("failed to receive control message")?;
					self.handle_message(msg).await?;
				},
				res = self.tasks.join_next(), if !self.tasks.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::error!("failed to serve subscription: {:?}", err);
					}
				},
				delta = broadcasts.next() => {
					let delta = delta.expect("no more broadcasts");
					self.handle_broadcast(delta).await?;
				},
			}
		}
	}

	async fn handle_broadcast(&mut self, delta: broadcasts::Delta) -> anyhow::Result<()> {
		match delta {
			broadcasts::Delta::Insert(name, _broadcast) => {
				self.send_message(control::Announce {
					track_namespace: name.clone(),
				})
				.await
			}
			broadcasts::Delta::Remove(name) => {
				self.send_message(control::AnnounceError {
					track_namespace: name,
					code: VarInt::from_u32(0),
					reason: "broadcast closed".to_string(),
				})
				.await
			}
		}
	}

	async fn handle_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		log::info!("received message: {:?}", msg);

		// TODO implement publish and subscribe halves of the protocol.
		match msg {
			control::Message::Announce(msg) => self.receive_announce(msg).await,
			control::Message::AnnounceOk(msg) => self.receive_announce_ok(msg),
			control::Message::AnnounceError(msg) => self.receive_announce_error(msg),
			control::Message::Subscribe(msg) => self.receive_subscribe(msg).await,
			control::Message::SubscribeOk(msg) => self.receive_subscribe_ok(msg),
			control::Message::SubscribeError(msg) => self.receive_subscribe_error(msg),
			control::Message::GoAway(_) => anyhow::bail!("client can't send GOAWAY u nerd"),
		}
	}

	async fn send_message<T: Into<control::Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);
		self.control.send(msg).await
	}

	async fn receive_announce(&mut self, msg: control::Announce) -> anyhow::Result<()> {
		match self.announce(&msg) {
			Ok(()) => {
				self.send_message(control::AnnounceOk {
					track_namespace: msg.track_namespace,
				})
				.await
			}
			Err(e) => {
				self.send_message(control::AnnounceError {
					track_namespace: msg.track_namespace,
					code: VarInt::from_u32(1),
					reason: e.to_string(),
				})
				.await
			}
		}
	}

	fn receive_announce_ok(&mut self, _msg: control::AnnounceOk) -> anyhow::Result<()> {
		Ok(())
	}

	fn receive_announce_error(&mut self, msg: control::AnnounceError) -> anyhow::Result<()> {
		// TODO make sure we sent this announce
		anyhow::bail!("received ANNOUNCE_ERROR({:?}): {}", msg.code, msg.reason)
	}

	async fn receive_subscribe(&mut self, msg: control::Subscribe) -> anyhow::Result<()> {
		match self.subscribe(&msg) {
			Ok(()) => {
				self.send_message(control::SubscribeOk {
					track_id: msg.track_id,
					expires: None,
				})
				.await
			}
			Err(e) => {
				self.send_message(control::SubscribeError {
					track_id: msg.track_id,
					code: VarInt::from_u32(1),
					reason: e.to_string(),
				})
				.await
			}
		}
	}

	fn receive_subscribe_ok(&mut self, _msg: control::SubscribeOk) -> anyhow::Result<()> {
		Ok(())
	}

	fn receive_subscribe_error(&mut self, msg: control::SubscribeError) -> anyhow::Result<()> {
		// TODO make sure we sent this subscribe
		anyhow::bail!("received SUBSCRIBE_ERROR({:?}): {}", msg.code, msg.reason)
	}

	fn announce(&mut self, msg: &control::Announce) -> anyhow::Result<()> {
		let broadcast = Broadcast::default();

		let mut broadcasts = self.broadcasts.lock().unwrap();

		if broadcasts.contains_key(&msg.track_namespace) {
			anyhow::bail!("duplicate broadcast: {}", msg.track_namespace);
		}

		broadcasts.insert(msg.track_namespace.clone(), broadcast);

		Ok(())
	}

	fn subscribe(&mut self, msg: &control::Subscribe) -> anyhow::Result<()> {
		let broadcasts = self.broadcasts.lock().unwrap();

		let broadcast = broadcasts
			.get(&msg.track_namespace)
			.context("unknown track namespace")?;

		let track = broadcast
			.tracks
			.get(&msg.track_name)
			.context("unknown track name")?
			.clone();

		let track_id = msg.track_id;

		let sub = Subscription {
			track,
			track_id,
			transport: self.transport.clone(),
		};

		self.tasks.spawn(async move { sub.serve().await });

		Ok(())
	}
}

pub struct Subscription {
	transport: Arc<object::Transport>,
	track_id: VarInt,
	track: Track,
}

impl Subscription {
	pub async fn serve(mut self) -> anyhow::Result<()> {
		let mut tasks = JoinSet::new();
		let mut done = false;

		loop {
			tokio::select! {
				// Accept new tracks added to the broadcast.
				segment = self.track.segments.next(), if !done => {
					match segment {
						Some(segment) => {
							let group = Group {
								segment,
								transport: self.transport.clone(),
								track_id: self.track_id,
							};

							tasks.spawn(async move { group.serve().await });
						},
						None => done = true, // no more segments in the track
					}
				},
				// Poll any pending segments until they exit.
				res = tasks.join_next(), if !tasks.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					res.context("failed serve segment")?
				},
				else => return Ok(()), // all segments received and finished serving
			}
		}
	}
}

struct Group {
	transport: Arc<object::Transport>,
	track_id: VarInt,
	segment: Segment,
}

impl Group {
	pub async fn serve(mut self) -> anyhow::Result<()> {
		let header = object::Header {
			track_id: self.track_id,
			group_sequence: self.segment.sequence,
			object_sequence: VarInt::from_u32(0), // Always zero since we send an entire group as an object
			send_order: self.segment.send_order,
		};

		let mut stream = self.transport.send(header).await?;

		// Write each fragment as they are available.
		while let Some(fragment) = self.segment.fragments.next().await {
			stream.write_all(fragment.as_slice()).await?;
		}

		// NOTE: stream is automatically closed when dropped

		Ok(())
	}
}
