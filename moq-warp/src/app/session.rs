use crate::media;

use anyhow::Context;

use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use std::sync::Arc;

use moq_transport::{control, data, server, setup};

pub struct Session {
	// Used to send/receive data streams.
	transport: Arc<data::Transport>,

	// Used to send/receive control messages.
	control: control::Stream,

	// The list of available broadcasts for the session.
	media: media::Broadcasts,
}

impl Session {
	pub async fn accept(session: server::Accept, media: media::Broadcasts) -> anyhow::Result<Session> {
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
		};

		Ok(session)
	}

	pub async fn serve(mut self) -> anyhow::Result<()> {
		loop {
			for name in self.media.keys() {
				// Announce each available broadcast.
				let msg = control::Announce {
					track_namespace: name.clone(),
					auth: None,
				};

				self.control.send(msg.into()).await?;
			}

			// TODO implement publish and subscribe halves of the protocol.
			let msg = self.control.recv().await?;
			match msg {
				control::Message::Announce(_) => anyhow::bail!("publishing not supported"),
				control::Message::AnnounceOk(ref _ok) => {
					// noop
				}
				control::Message::AnnounceError(ref err) => {
					anyhow::bail!("received ANNOUNCE_ERROR({:?}): {}", err.code, err.reason);
				}
				control::Message::Subscribe(ref sub) => match self.subscribe(sub) {
					Ok(()) => {
						let msg = control::SubscribeOk {
							track_id: sub.track_id,
							expires: None,
						};

						self.control.send(msg.into()).await?;
					}
					Err(e) => {
						let msg = control::SubscribeError {
							track_id: sub.track_id,
							code: 1,
							reason: e.to_string(),
						};

						self.control.send(msg.into()).await?;
					}
				},
				control::Message::SubscribeOk(_) => anyhow::bail!("publishing not supported"),
				control::Message::SubscribeError(_) => anyhow::bail!("publishing not supported"),
				control::Message::GoAway(_) => anyhow::bail!("goaway not supported"),
			}
		}
	}

	fn subscribe(&mut self, sub: &control::Subscribe) -> anyhow::Result<()> {
		anyhow::ensure!(sub.group_sequence.is_none(), "group sequence not supported");
		anyhow::ensure!(sub.object_sequence.is_none(), "object sequence not supported");

		let broadcast = self
			.media
			.get(&sub.track_namespace)
			.context("unknown track namespace")?;

		let track = broadcast
			.tracks
			.get(&sub.track_name)
			.context("unknown track name")?
			.clone();

		let sub = Subscription {
			track,
			track_id: sub.track_id,
			transport: self.transport.clone(),
		};
		tokio::spawn(async move { sub.serve().await });

		Ok(())
	}
}

pub struct Subscription {
	transport: Arc<data::Transport>,
	track_id: u64,
	track: media::Track,
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

							tasks.spawn(async move {
								group.serve().await
							});
						},
						None => done = true,
					}
				},
				// Poll any pending segments until they exit.
				res = tasks.join_next(), if !tasks.is_empty() => {
					let res = res.context("no tasks running")?;
					let res = res.context("failed to run segment")?;
					res.context("failed serve segment")?
				},
				else => return Ok(()),
			}
		}
	}
}

struct Group {
	transport: Arc<data::Transport>,
	track_id: u64,
	segment: media::Segment,
}

impl Group {
	pub async fn serve(mut self) -> anyhow::Result<()> {
		// TODO proper values
		let header = moq_transport::data::Header {
			track_id: self.track_id,
			group_sequence: 0,
			object_sequence: 0,
			send_order: 0,
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
