use crate::{data, session::SessionError};

use super::Subscribe;

pub struct TrackHeader {
	pub send_order: u64,
}

pub struct TrackWriter {
	subscribe: Subscribe,
	stream: webtransport_quinn::SendStream,

	max: Option<(u64, u64)>,
	remain: usize,
}

pub struct TrackChunk {
	pub group_id: u64,
	pub object_id: u64,
	pub size: usize,
}

impl TrackWriter {
	pub(crate) fn new(subscribe: Subscribe, stream: webtransport_quinn::SendStream) -> Self {
		Self {
			subscribe,
			stream,
			max: None,
			remain: 0,
		}
	}

	pub async fn write(&mut self, chunk: TrackChunk, payload: &[u8]) -> Result<(), SessionError> {
		self.write_chunk(chunk).await?;
		self.write_payload(payload).await?;

		Ok(())
	}

	// Advanced method to avoid buffering the entire payload.
	pub async fn write_chunk(&mut self, chunk: TrackChunk) -> Result<(), SessionError> {
		// Make sure you don't screw up the size.
		if self.remain > 0 {
			return Err(SessionError::InvalidSize);
		}

		if let Some((max_group, max_object)) = self.max {
			if chunk.group_id < max_group {
				return Err(SessionError::OutOfOrder);
			}

			if chunk.object_id <= max_object {
				return Err(SessionError::OutOfOrder);
			}
		}

		self.max = Some((chunk.group_id, chunk.object_id));

		let msg = data::TrackChunk {
			group_id: chunk.group_id,
			object_id: chunk.object_id,
			size: chunk.size,
		};

		self.subscribe.serve(chunk.group_id, chunk.object_id)?;

		msg.encode(&mut self.stream).await?;
		self.remain = chunk.size;

		Ok(())
	}

	// Called after write_chunk with the payload, which MUST eventually equal the same size.
	pub async fn write_payload(&mut self, payload: &[u8]) -> Result<(), SessionError> {
		if self.remain < payload.len() {
			return Err(SessionError::WrongSize);
		}

		self.stream.write_all(payload).await?;
		self.remain -= payload.len();

		Ok(())
	}
}
