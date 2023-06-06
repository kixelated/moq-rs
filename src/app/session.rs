use crate::{media, transport};

use anyhow::Context;

use std::sync::Arc;
use tokio::task::JoinSet;

use tokio::io::AsyncWriteExt;

use super::message;

#[derive(Clone)]
pub struct Session {
	// The underlying transport session
	transport: Arc<transport::Session>,
}

impl Session {
	pub fn new(transport: transport::Session) -> Self {
		let transport = Arc::new(transport);
		Self { transport }
	}

	pub async fn serve_broadcast(&self, mut broadcast: media::broadcast::Subscriber) -> anyhow::Result<()> {
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

	pub async fn serve_track(&self, mut track: media::track::Subscriber) -> anyhow::Result<()> {
		log::info!("serving track");
		let mut tasks = JoinSet::new();
		let mut fin = false;

		let track_id = track.id();

		loop {
			tokio::select! {
				// Accept new tracks added to the broadcast.
				segment = track.next_segment(), if !fin => {
					log::info!("next segment: {:?}", segment.is_ok());
					match segment.context("failed to get next segment")? {
						Some(segment) => {
							let session = self.clone();

							tasks.spawn(async move {
								session.serve_segment(track_id, segment).await
							});
						},
						None => {
							log::info!("no more segments");
							fin = true;
						},
					}
				},
				// Poll any pending segments until they exit.
				res = tasks.join_next(), if !tasks.is_empty() => {
					log::info!("join_next: {:?}", res);
					let res = res.context("no tasks running")?;
					let res = res.context("failed to run segment")?;
					res.context("failed serve segment")?
				},
				else => return Ok(()),
			}
		}
	}

	pub async fn serve_segment(&self, track_id: u32, mut segment: media::segment::Subscriber) -> anyhow::Result<()> {
		log::info!("serving segment: {}", track_id);
		let mut stream = self.transport.open_uni(self.transport.session_id()).await?;

		// TODO support prioirty
		// stream.set_priority(0);

		// Encode a JSON header indicating this is a new track.
		let mut message: message::Message = message::Message::new();
		message.segment = Some(message::Segment { track_id });

		// Write the JSON header.
		let data = message.serialize()?;
		stream.write_all(data.as_slice()).await?;

		// Write each fragment as they are available.
		while let Some(fragment) = segment.next_fragment().await? {
			stream.write_all(fragment.as_slice()).await?;
		}

		// TODO support closing streams
		// stream.finish().await?;

		Ok(())
	}
}

/*
	fn poll_media(&mut self) -> anyhow::Result<Option<time::Duration>> {
		for (track_id, track_sub) in &mut self.tracks {
			while let Some(segment) = track_sub.segment() {
				// Create a new unidirectional stream.
				let stream_id = self.session.open_stream(&mut self.quic, false)?;

				// TODO send order
				// Set the stream priority to be equal to the timestamp.
				// We subtract from u64::MAX so newer media is sent important.
				// TODO prioritize audio
				// let order = u64::MAX - fragment.timestamp;
				// self.streams.send_order(conn, stream_id, order);

				// Encode a JSON header indicating this is a new track.
				let mut message: message::Message = message::Message::new();
				message.segment = Some(message::Segment { track_id: *track_id });

				// Write the header.
				let data = message.serialize()?;
				self.streams.send(&mut self.quic, stream_id, &data, false)?;

				let segment = media::segment::Subscriber::new(segment);
				self.segments.insert(stream_id, segment);
			}
		}

		self.tracks.retain(|_, track_sub| !track_sub.done());

		for (stream_id, segment_sub) in &mut self.segments {
			while let Some(fragment) = segment_sub.fragment() {
				// Write the fragment.
				self.streams
					.send(&mut self.quic, *stream_id, fragment.as_slice(), false)?;
			}

			// TODO combine with the retain call below
			if segment_sub.done() {
				// Close the stream
				self.streams.send(&mut self.quic, *stream_id, &[], true)?;
			}
		}

		self.segments.retain(|_stream_id, segment_sub| !segment_sub.done());

		// TODO implement futures...
		Ok(time::Duration::from_millis(10).into())
	}
}
	*/
