use std::{mem, ops::Deref};

use crate::{coding::DecodeError, data, error::ReadError};

use super::Subscribe;
use bytes::{Bytes, BytesMut};
use paste::paste;

pub struct Object {
	pub header: ObjectHeader,
	pub payload: Bytes,
}

impl Deref for Object {
	type Target = ObjectHeader;

	fn deref(&self) -> &Self::Target {
		&self.header
	}
}

pub struct ObjectHeader {
	pub group_id: u64,
	pub object_id: u64,
	pub send_order: u64,
	pub size: usize,
}

macro_rules! stream_types {
    {$($name:ident,)*} => {
		/// A stream of objects, potentially in the same group.
		// This is not a trait because 1. it's async and 2. we need to know the type.
		pub enum Stream {
			$($name(paste! { [<$name Stream>] })),*
		}

		impl Stream {
			pub fn new(subscribe: Subscribe, header: data::Header, stream: webtransport_quinn::RecvStream) -> Self {
				match header {
					$(data::Header::$name(header) => <paste! { [< $name Stream >] }>::new(subscribe, header, stream).into(),)*
				}
			}

			/// The associated subscription.
			pub fn subscription(&self) -> &Subscribe {
				match self {
					$(Self::$name(s) => s.subscription(),)*
				}
			}

			/// Read the next object, including the payload.
			pub async fn read(&mut self) -> Result<Option<Object>, ReadError> {
				match self {
					$(Self::$name(s) => s.read().await,)*
				}
			}

			/// Read the next object header, without the payload (lower latency).
			pub async fn read_header(&mut self) -> Result<Option<ObjectHeader>, ReadError> {
				match self {
					$(Self::$name(s) => s.read_header().await,)*
				}
			}

			/// Read the payload from the previous object, returning None when finished.
			pub async fn read_payload(&mut self) -> Result<Option<Bytes>, ReadError> {
				match self {
					$(Self::$name(s) => s.read_payload().await,)*
				}
			}
		}

		$(impl From<paste! { [<$name Stream>] }> for Stream{
			fn from(m: paste! { [<$name Stream>] }) -> Self {
				Self::$name(m)
			}
		})*
    }
}

stream_types! {
	Object,
	Group,
	Track,
}

/// A stream of objects, in the same track but otherwise ungrouped.
pub struct TrackStream {
	subscribe: Subscribe,
	header: data::TrackHeader,
	stream: webtransport_quinn::RecvStream,

	remain: usize,
}

impl TrackStream {
	pub fn new(subscribe: Subscribe, header: data::TrackHeader, stream: webtransport_quinn::RecvStream) -> Self {
		Self {
			subscribe,
			header,
			stream,
			remain: 0,
		}
	}

	pub fn send_order(&self) -> u64 {
		self.header.send_order
	}

	pub fn subscription(&self) -> &Subscribe {
		&self.subscribe
	}

	pub async fn read(&mut self) -> Result<Option<Object>, ReadError> {
		let header = match self.read_header().await? {
			Some(header) => header,
			None => return Ok(None),
		};

		// Concatenate the payload chunks into one big slice.
		let mut payload = BytesMut::new();
		while let Some(chunk) = self.read_payload().await? {
			payload.extend_from_slice(&chunk);
		}

		let object = Object {
			header,
			payload: payload.freeze(),
		};

		Ok(Some(object))
	}

	pub async fn read_header(&mut self) -> Result<Option<ObjectHeader>, ReadError> {
		if self.remain != 0 {
			return Err(ReadError::Short);
		}

		let header = match data::TrackChunk::decode(&mut self.stream).await {
			Ok(chunk) => chunk,
			Err(DecodeError::UnexpectedEnd) => return Ok(None),
			Err(e) => return Err(e.into()),
		};

		let header = ObjectHeader {
			group_id: header.group_id,
			object_id: header.object_id,
			send_order: self.header.send_order,
			size: header.size,
		};

		self.subscribe.seen(header.group_id, header.object_id).ok();
		self.remain = header.size;

		Ok(Some(header))
	}

	pub async fn read_payload(&mut self) -> Result<Option<Bytes>, ReadError> {
		if self.remain == 0 {
			return Ok(None);
		}

		let chunk = self
			.stream
			.read_chunk(self.remain, true)
			.await?
			.ok_or(ReadError::Short)?;

		self.remain -= chunk.bytes.len();

		Ok(Some(chunk.bytes))
	}
}

pub struct GroupStream {
	subscribe: Subscribe,
	header: data::GroupHeader,
	stream: webtransport_quinn::RecvStream,
	remain: usize,
}

impl GroupStream {
	pub fn new(subscribe: Subscribe, header: data::GroupHeader, stream: webtransport_quinn::RecvStream) -> Self {
		Self {
			subscribe,
			header,
			stream,
			remain: 0,
		}
	}

	pub fn group_id(&self) -> u64 {
		self.header.group_id
	}

	pub fn subscription(&self) -> &Subscribe {
		&self.subscribe
	}

	pub async fn read(&mut self) -> Result<Option<Object>, ReadError> {
		let header = match self.read_header().await? {
			Some(header) => header,
			None => return Ok(None),
		};

		// Concatenate the payload chunks into one big slice.
		let mut payload = BytesMut::new();
		while let Some(chunk) = self.read_payload().await? {
			payload.extend_from_slice(&chunk);
		}

		let object = Object {
			header,
			payload: payload.freeze(),
		};

		Ok(Some(object))
	}

	pub async fn read_header(&mut self) -> Result<Option<ObjectHeader>, ReadError> {
		if self.remain != 0 {
			return Err(ReadError::Short);
		}

		let header = match data::GroupChunk::decode(&mut self.stream).await {
			Ok(header) => header,
			Err(DecodeError::UnexpectedEnd) => return Ok(None),
			Err(err) => return Err(err.into()),
		};

		let header = ObjectHeader {
			group_id: self.header.group_id,
			object_id: header.object_id,
			send_order: self.header.send_order,
			size: header.size,
		};

		self.subscribe.seen(header.group_id, header.object_id).ok();
		self.remain = header.size;

		Ok(Some(header))
	}

	pub async fn read_payload(&mut self) -> Result<Option<Bytes>, ReadError> {
		if self.remain == 0 {
			return Ok(None);
		}

		let chunk = self
			.stream
			.read_chunk(self.remain, true)
			.await?
			.ok_or(ReadError::Short)?;

		self.remain -= chunk.bytes.len();

		Ok(Some(chunk.bytes))
	}
}

// Crude way of emulating a stream with a single element.
enum ObjectState {
	Header,
	Payload(Bytes),
	Done,
}

pub struct ObjectStream {
	subscribe: Subscribe,
	header: data::ObjectHeader,
	stream: webtransport_quinn::RecvStream,

	state: ObjectState,
}

impl ObjectStream {
	pub fn new(subscribe: Subscribe, header: data::ObjectHeader, stream: webtransport_quinn::RecvStream) -> Self {
		Self {
			subscribe,
			header,
			stream,
			state: ObjectState::Header,
		}
	}

	pub fn group_id(&self) -> u64 {
		self.header.group_id
	}

	pub fn object_id(&self) -> u64 {
		self.header.object_id
	}

	pub fn send_order(&self) -> u64 {
		self.header.send_order
	}

	pub fn subscription(&self) -> &Subscribe {
		&self.subscribe
	}

	pub async fn read(&mut self) -> Result<Option<Object>, ReadError> {
		let header = match self.read_header().await? {
			Some(header) => header,
			None => return Ok(None),
		};

		let payload = match mem::replace(&mut self.state, ObjectState::Done) {
			ObjectState::Payload(payload) => payload,
			_ => unreachable!(),
		};

		let object = Object {
			header,
			payload: payload.clone(),
		};

		Ok(Some(object))
	}

	pub async fn read_header(&mut self) -> Result<Option<ObjectHeader>, ReadError> {
		match self.state {
			ObjectState::Header => {}
			ObjectState::Payload(_) => return Err(ReadError::Short),
			ObjectState::Done => return Ok(None),
		};

		// Concatenate the payload chunks into one big slice.
		let mut payload = BytesMut::new();
		while let Some(chunk) = self.stream.read_chunk(usize::MAX, true).await? {
			payload.extend_from_slice(&chunk.bytes);
		}

		let header = ObjectHeader {
			group_id: self.header.group_id,
			object_id: self.header.object_id,
			send_order: self.header.send_order,
			size: payload.len(),
		};

		self.subscribe.seen(header.group_id, header.object_id).ok();

		// Save the payload for the read_payload calls.
		self.state = ObjectState::Payload(payload.freeze());

		Ok(Some(header))
	}

	pub async fn read_payload(&mut self) -> Result<Option<Bytes>, ReadError> {
		// TODO error if you call read_payload without read_header

		let state = mem::replace(&mut self.state, ObjectState::Done);
		Ok(match state {
			ObjectState::Payload(payload) => Some(payload),
			_ => None,
		})
	}
}
