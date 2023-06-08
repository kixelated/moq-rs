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

	pub async fn serve_broadcast(&self, mut broadcast: media::broadcast::Consumer) -> anyhow::Result<()> {
		let mut tasks = JoinSet::new();

		loop {
			tokio::select! {
				// Accept new tracks added to the broadcast.
				track = broadcast.next_track() => {
					let track = track.context("failed to accept track")?;
					let session = self.clone();

					tasks.spawn(async move {
						session.serve_track(track).await
					});
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

	pub async fn serve_track(&self, mut track: media::track::Consumer) -> anyhow::Result<()> {
		let mut tasks = JoinSet::new();
		let mut fin = false;

		let track_id = track.id();

		loop {
			tokio::select! {
				// Accept new tracks added to the broadcast.
				segment = track.next_segment(), if !fin => {
					match segment {
						Some(segment) => {
							let session = self.clone();
							tasks.spawn(async move {
								session.serve_segment(track_id, segment).await
							});
						},
						None => {
							// No more segments, but keep looping until tasks are empty
							fin = true;
						},
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

	pub async fn serve_segment(&self, track_id: u32, mut segment: media::segment::Consumer) -> anyhow::Result<()> {
		let mut stream = self.transport.open_uni(self.transport.session_id()).await?;

		// TODO support prioirty
		// stream.set_priority(0);

		// Encode a JSON header indicating this is a new segment.
		let mut message: message::Message = message::Message::new();

		// TODO combine init and segment messages into one.
		if track_id == 0xff {
			message.init = Some(message::Init {});
		} else {
			message.segment = Some(message::Segment { track_id });
		}

		// Write the JSON header.
		let data = message.serialize()?;
		stream.write_all(data.as_slice()).await?;

		// Write each fragment as they are available.
		while let Some(fragment) = segment.next_fragment().await {
			stream.write_all(fragment.as_slice()).await?;
		}

		// NOTE: stream is automatically closed when dropped

		Ok(())
	}
}
