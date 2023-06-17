use anyhow::Context;

use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use std::sync::Arc;

use moq_transport::coding::VarInt;
use moq_transport::{control, data, server, setup};
use moq_warp::{Broadcasts, Segment, Track};

pub struct Session {
	// Used to send/receive data streams.
	transport: Arc<data::Transport>,

	// Used to send/receive control messages.
	control: control::Stream,

	// The list of available broadcasts for the session.
	media: Broadcasts,

	// The list of active subscriptions.
	tasks: JoinSet<anyhow::Result<()>>,
}

impl Session {
	pub async fn accept(session: server::Accept, media: Broadcasts) -> anyhow::Result<Session> {
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
			media,
			tasks: JoinSet::new(),
		};

		Ok(session)
	}

	pub async fn serve(mut self) -> anyhow::Result<()> {
		// TODO fix lazy: make a copy of the strings to avoid the borrow checker on self.
		let broadcasts: Vec<String> = self.media.keys().cloned().collect();

		// Announce each available broadcast immediately.
		for name in broadcasts {
			self.send_message(control::Announce {
				track_namespace: name.clone(),
			})
			.await?;
		}

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
				}
			}
		}
	}

	async fn handle_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		log::info!("received message: {:?}", msg);

		// TODO implement publish and subscribe halves of the protocol.
		match msg {
			control::Message::Announce(_) => anyhow::bail!("ANNOUNCE not supported"),
			control::Message::AnnounceOk(ref _ok) => Ok(()), // noop
			control::Message::AnnounceError(ref err) => {
				anyhow::bail!("received ANNOUNCE_ERROR({:?}): {}", err.code, err.reason)
			}
			control::Message::Subscribe(ref sub) => self.receive_subscribe(sub).await,
			control::Message::SubscribeOk(_) => anyhow::bail!("SUBSCRIBE OK not supported"),
			control::Message::SubscribeError(_) => anyhow::bail!("SUBSCRIBE ERROR not supported"),
			control::Message::GoAway(_) => anyhow::bail!("goaway not supported"),
		}
	}

	async fn send_message<T: Into<control::Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);
		self.control.send(msg).await
	}

	async fn receive_subscribe(&mut self, sub: &control::Subscribe) -> anyhow::Result<()> {
		match self.subscribe(sub) {
			Ok(()) => {
				self.send_message(control::SubscribeOk {
					track_id: sub.track_id,
					expires: None,
				})
				.await
			}
			Err(e) => {
				self.send_message(control::SubscribeError {
					track_id: sub.track_id,
					code: VarInt::from_u32(1),
					reason: e.to_string(),
				})
				.await
			}
		}
	}

	fn subscribe(&mut self, sub: &control::Subscribe) -> anyhow::Result<()> {
		let broadcast = self
			.media
			.get(&sub.track_namespace)
			.context("unknown track namespace")?;

		let track = broadcast
			.tracks
			.get(&sub.track_name)
			.context("unknown track name")?
			.clone();

		let track_id = sub.track_id;

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
	transport: Arc<data::Transport>,
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
	transport: Arc<data::Transport>,
	track_id: VarInt,
	segment: Segment,
}

impl Group {
	pub async fn serve(mut self) -> anyhow::Result<()> {
		let header = data::Header {
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
