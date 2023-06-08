use crate::media;

use anyhow::Context;

use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;

use super::WebTransportSession;

use super::message;

#[derive(Clone)]
pub struct Session {
	// The underlying transport session
	transport: Arc<WebTransportSession>,
}

impl Session {
	pub fn new(transport: WebTransportSession) -> Self {
		let transport = Arc::new(transport);
		Self { transport }
	}

	pub async fn serve_broadcast(&self, mut broadcast: media::Broadcast) -> anyhow::Result<()> {
		let mut tasks = JoinSet::new();
		let mut done = false;

		loop {
			tokio::select! {
				// Accept new tracks added to the broadcast.
				track = broadcast.tracks.next(), if !done => {
					match track {
						Some(track) => {
							let session = self.clone();

							tasks.spawn(async move {
								session.serve_track(track).await
							});
						},
						None => done = true,
					}
				},
				// Poll any pending tracks until they exit.
				res = tasks.join_next(), if !tasks.is_empty() => {
					let res = res.context("no tracks running")?;
					let res = res.context("failed to run track")?;
					res.context("failed to serve track")?;
				},
				else => return Ok(()),
			}
		}
	}

	pub async fn serve_track(&self, mut track: media::Track) -> anyhow::Result<()> {
		let mut tasks = JoinSet::new();
		let mut done = false;

		loop {
			tokio::select! {
				// Accept new tracks added to the broadcast.
				segment = track.segments.next(), if !done => {
					match segment {
						Some(segment) => {
							let track = track.clone();
							let session = self.clone();

							tasks.spawn(async move {
								session.serve_segment(track, segment).await
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

	pub async fn serve_segment(&self, track: media::Track, mut segment: media::Segment) -> anyhow::Result<()> {
		let mut stream = self.transport.open_uni(self.transport.session_id()).await?;

		// TODO support prioirty
		// stream.set_priority(0);

		// Encode a JSON header indicating this is a new segment.
		let mut message: message::Message = message::Message::new();

		// TODO combine init and segment messages into one.
		if track.id == 0xff {
			message.init = Some(message::Init {});
		} else {
			message.segment = Some(message::Segment { track_id: track.id });
		}

		// Write the JSON header.
		let data = message.serialize()?;
		stream.write_all(data.as_slice()).await?;

		// Write each fragment as they are available.
		while let Some(fragment) = segment.fragments.next().await {
			stream.write_all(fragment.as_slice()).await?;
		}

		// NOTE: stream is automatically closed when dropped

		Ok(())
	}
}
