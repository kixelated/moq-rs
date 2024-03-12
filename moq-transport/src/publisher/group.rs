use crate::{data, session::SessionError};

use super::Subscribe;

pub struct GroupHeader {
	pub group_id: u64,
	pub send_order: u64,
}

pub struct GroupWriter {
	subscribe: Subscribe,
	stream: webtransport_quinn::SendStream,
	group_id: u64,

	max: Option<u64>,
	remain: usize,
}

pub struct GroupChunk {
	pub object_id: u64,
	pub size: usize,
}

impl GroupWriter {
	pub(crate) fn new(subscribe: Subscribe, stream: webtransport_quinn::SendStream, group_id: u64) -> Self {
		Self {
			subscribe,
			stream,
			group_id,
			max: None,
			remain: 0,
		}
	}

	// Use this 99% of the time.
	pub async fn write(&mut self, payload: &[u8]) -> Result<(), SessionError> {
		let next = match self.max {
			Some(max) => max + 1,
			None => 0,
		};

		let chunk = GroupChunk {
			object_id: next,
			size: payload.len(),
		};

		self.write_chunk(chunk).await?;
		self.write_payload(payload).await?;

		Ok(())
	}

	// Advanced method to avoid buffering the entire payload.
	pub async fn write_chunk(&mut self, chunk: GroupChunk) -> Result<(), SessionError> {
		// Make sure you don't screw up the size.
		if self.remain > 0 {
			return Err(SessionError::InvalidSize);
		}

		if let Some(max) = self.max {
			if chunk.object_id <= max {
				return Err(SessionError::OutOfOrder);
			}
		}

		self.max = Some(chunk.object_id);

		self.subscribe.serve(self.group_id, chunk.object_id)?;

		let msg = data::GroupChunk {
			object_id: chunk.object_id,
			size: chunk.size,
		};

		msg.encode(&mut self.stream).await?;
		self.remain = chunk.size;

		Ok(())
	}

	// Called after write_chunk with the payload, which MUST eventually equal the specified size.
	pub async fn write_payload(&mut self, payload: &[u8]) -> Result<(), SessionError> {
		if self.remain < payload.len() {
			return Err(SessionError::InvalidSize);
		}

		self.stream.write_all(payload).await?;
		self.remain -= payload.len();

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), SessionError> {
		self.subscribe.closed()
	}

	pub async fn reset(mut self, code: u32) -> Result<(), SessionError> {
		self.stream.reset(code);
		Ok(())
	}
}

impl Drop for GroupWriter {
	fn drop(&mut self) {
		if self.remain > 0 {
			// TODO specify an error code
			self.stream.reset(1);
		}
	}
}
