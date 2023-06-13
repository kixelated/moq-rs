use crate::media;

use anyhow::Context;

use std::sync::Arc;
use tokio::task::JoinSet;

use tokio::io::AsyncWriteExt;

#[derive(Clone)]
pub struct Session {
	// The underlying MoQ transport session
	transport: Arc<moq_transport::server::Session>,
}

impl Session {
	pub fn new(transport: moq_transport::server::Session) -> Self {
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
		// TODO proper values
		let header = moq_transport::object::Header {
			track_id: track.id,
			group_sequence: 0,
			object_sequence: 0,
			send_order: 0,
		};

		let mut stream = self.transport.send_data(header).await?;

		// Write each fragment as they are available.
		while let Some(fragment) = segment.fragments.next().await {
			stream.write_all(fragment.as_slice()).await?;
		}

		// NOTE: stream is automatically closed when dropped

		Ok(())
	}
}
