use crate::{data, error::WriteError};

use super::Subscribe;

pub struct GroupHeader {
	pub group_id: u64,
	pub send_order: u64,
}

pub struct GroupStream {
	subscribe: Subscribe,
	stream: webtransport_quinn::SendStream,
	group_id: u64,

	max: Option<u64>,
	remain: usize,
}

pub struct GroupObject {
	pub object_id: u64,
	pub size: usize,
}

impl GroupStream {
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
	pub async fn write(&mut self, payload: &[u8]) -> Result<(), WriteError> {
		let next = match self.max {
			Some(max) => max + 1,
			None => 0,
		};

		let object = GroupObject {
			object_id: next,
			size: payload.len(),
		};

		self.write_object(object).await?;
		self.write_payload(payload).await?;

		Ok(())
	}

	// Advanced method to avoid buffering the entire payload.
	pub async fn write_object(&mut self, header: GroupObject) -> Result<(), WriteError> {
		// Make sure you don't screw up the size.
		if self.remain > 0 {
			return Err(WriteError::WrongSize);
		}

		if let Some(max) = self.max {
			if header.object_id <= max {
				return Err(WriteError::WrongOrder);
			}
		}

		self.subscribe.update_max(self.group_id, header.object_id)?;

		self.max = Some(header.object_id);
		self.remain = header.size;

		let msg = data::GroupChunk {
			object_id: header.object_id,
			size: header.size,
		};

		msg.encode(&mut self.stream).await?;

		Ok(())
	}

	// Called after write_object with the payload, which MUST eventually equal the specified size.
	pub async fn write_payload(&mut self, payload: &[u8]) -> Result<(), WriteError> {
		if self.remain < payload.len() {
			return Err(WriteError::WrongSize);
		}

		self.stream.write_all(payload).await?;
		self.remain -= payload.len();

		Ok(())
	}
}

impl Drop for GroupStream {
	fn drop(&mut self) {
		if self.remain > 0 {
			// TODO specify an error code
			self.stream.reset(1).ok();
		}
	}
}

pub struct TrackHeader {
	pub send_order: u64,
}

pub struct TrackStream {
	subscribe: Subscribe, // Keep the subscription alive while writing.
	stream: webtransport_quinn::SendStream,

	max: Option<(u64, u64)>,
	remain: usize,
}

pub struct TrackObject {
	pub group_id: u64,
	pub object_id: u64,
	pub size: usize,
}

impl TrackStream {
	pub(crate) fn new(subscribe: Subscribe, stream: webtransport_quinn::SendStream) -> Self {
		Self {
			subscribe,
			stream,
			max: None,
			remain: 0,
		}
	}

	// Advanced method to avoid buffering the entire payload.
	pub async fn write_object(&mut self, object: TrackObject) -> Result<(), WriteError> {
		// Make sure you don't screw up the size.
		if self.remain > 0 {
			return Err(WriteError::WrongSize);
		}

		if let Some((max_group, max_object)) = self.max {
			if object.group_id < max_group {
				return Err(WriteError::WrongOrder);
			}

			if object.object_id <= max_object {
				return Err(WriteError::WrongOrder);
			}
		}

		self.subscribe.update_max(object.group_id, object.object_id)?;

		self.max = Some((object.group_id, object.object_id));
		self.remain = object.size;

		let msg = data::TrackChunk {
			group_id: object.group_id,
			object_id: object.object_id,
			size: object.size,
		};

		msg.encode(&mut self.stream).await?;

		Ok(())
	}

	// Called after write_header with the payload, which MUST eventually equal the same size.
	pub async fn write_payload(&mut self, payload: &[u8]) -> Result<(), WriteError> {
		if self.remain < payload.len() {
			return Err(WriteError::WrongSize);
		}

		self.stream.write_all(payload).await?;
		self.remain -= payload.len();

		Ok(())
	}
}

pub struct ObjectHeader {
	pub group_id: u64,
	pub object_id: u64,
	pub send_order: u64,
}

// Placeholder until there's an explicit size we should monitor.
pub struct ObjectStream {
	stream: webtransport_quinn::SendStream,
}

impl ObjectStream {
	pub(crate) fn new(stream: webtransport_quinn::SendStream) -> Self {
		Self { stream }
	}

	pub async fn write(&mut self, payload: &[u8]) -> Result<(), WriteError> {
		self.stream.write_all(payload).await?;
		Ok(())
	}
}
