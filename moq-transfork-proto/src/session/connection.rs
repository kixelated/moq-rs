use std::collections::{hash_map, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, DecodeError},
	message::{ControlType, DataType},
};

use derive_more::From;

use super::{Error, Publisher, StreamId, Subscriber};

#[derive(Default)]
pub struct Connection {
	streams: HashMap<StreamId, Stream>,
	publisher: Publisher,
	subscriber: Subscriber,
}

impl Connection {
	/// Receive stream data from the remote.
	pub fn recv<B: Buf>(&mut self, id: StreamId, buf: &mut B) -> Result<(), Error> {
		if buf.is_empty() {
			return Ok(());
		}

		let stream = match self.streams.entry(id) {
			hash_map::Entry::Occupied(entry) => entry.into_mut(),
			hash_map::Entry::Vacant(entry) => {
				let kind = match id.is_bi() {
					true => ControlType::decode(buf)?.into(),
					false => DataType::decode(buf)?.into(),
				};

				entry.insert(Stream::new(buf))
			}
		};

		let buffer = stream.buffer.take().ok_or(Error::Poisoned)?;
		let chain = buffer.chain(buf);

		let res = match stream.kind {
			StreamKind::Data(kind) => match kind {
				DataType::Group => self.subscriber.recv_group(id, buf),
			},
			StreamKind::Control(kind) => match kind {
				ControlType::Announce => self.publisher.recv_announce(id, buf),
				ControlType::Subscribe => self.publisher.recv_subscribe(id, buf),
				ControlType::Info => self.publisher.recv_info(id, buf),
				ControlType::Session => self.recv_session(id, buf),
				ControlType::Fetch => unimplemented!(),
			},
		};

		match res {
			Error::Coding(DecodeError::Short) => {
				buffer.extend(buf);
				stream.buffer = Some(buffer);
				Ok(())
			}
			res => res,
		}
	}

	/// Receive a stream close from the remote.
	pub fn recv_close(&mut self, stream: StreamId, code: Option<u8>) -> Result<(), Error> {
		todo!()
	}

	/// Return the next chunk of data to send, if any
	pub fn send<B: BufMut>(&mut self, buf: &mut B) -> Option<StreamId> {
		if let Some(stream) = self.publisher.send(buf) {
			return Some(stream);
		}

		self.subscriber.send(buf)
	}

	/// Return the next stream that should be closed, if any.
	pub fn send_close(&mut self) -> Option<(StreamId, Option<u8>)> {
		if let Some(stream) = self.publisher.send_close() {
			return Some(stream);
		}

		self.subscriber.send_close()
	}

	/// Return a handle to the publishing half.
	pub fn publisher(&mut self) -> &mut Publisher {
		&mut self.publisher
	}

	/// Return a handle to the subscribing half.
	pub fn subscriber(&mut self) -> &mut Publisher {
		&mut self.publisher
	}

	fn recv_session<B: Buf>(&mut self, id: StreamId, buf: &mut B) -> Result<(), Error> {
		todo!("perform handshake")
	}
}

#[derive(From)]
enum StreamKind {
	Control(ControlType),
	Data(DataType),
}

struct Stream {
	buffer: Option<VecDeque<u8>>,
	kind: StreamKind,
}

impl Stream {
	pub fn new(kind: StreamKind) -> Self {
		Self {
			buffer: Some(VecDeque::new()),
			kind,
		}
	}
}

impl Stream {
	pub fn recv(&mut self, mut buf: &[u8]) -> Result<(), Error> {
		let mut buffer = self.buffer.take().unwrap();

		loop {
			let mut chain = (&mut buffer).chain(&mut buf);

			match self.recv_once(&mut chain) {
				Ok(()) => (),
				Err(Error::Coding(DecodeError::Short)) => {
					buffer.extend(buf);
					self.buffer = Some(buffer);
					return Ok(());
				}
				Err(err) => return Err(err),
			}
		}
	}
}
