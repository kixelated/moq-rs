// WORK IN PROGRESS

use std::collections::{hash_map, HashMap};

use bytes::{Bytes, BytesMut};

use crate::generic;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("transfork error: {0}")]
	Generic(#[from] generic::Error),

	#[error("read error: {0}")]
	Read(#[from] quinn_proto::ReadError),

	#[error("write error: {0}")]
	Write(#[from] quinn_proto::WriteError),

	#[error("readable error: {0}")]
	Readable(#[from] quinn_proto::ReadableError),
}

pub struct Connection {
	generic: generic::Connection,

	buffer: BytesMut,
	buffered: HashMap<quinn_proto::StreamId, Bytes>,
}

impl Connection {
	pub fn client() -> Self {
		Self {
			generic: generic::Connection::client(),
			buffer: BytesMut::new(),
			buffered: HashMap::new(),
		}
	}

	pub fn server() -> Self {
		Self {
			generic: generic::Connection::server(),
			buffer: BytesMut::new(),
			buffered: HashMap::new(),
		}
	}

	pub fn poll(&mut self, quinn: &mut quinn_proto::Connection) -> Result<(), Error> {
		while let Some(event) = quinn.poll() {
			match event {
				quinn_proto::Event::Stream(event) => self.poll_stream(quinn, event)?,
				quinn_proto::Event::Connected => {
					// Create the initial bidirectional and unidirectional streams.
					self.open(quinn, quinn_proto::Dir::Bi);
					self.open(quinn, quinn_proto::Dir::Uni);
				}
				_ => todo!(),
			}
		}

		while let Some(event) = self.generic.poll() {
			match event {
				generic::ConnectionEvent::Stream(event) => match event {
					generic::StreamEvent::Open(dir) => self.open(quinn, dir.into()),
					generic::StreamEvent::Encodable(id) => self.encode(quinn, id.into())?,
				},
			}
		}

		Ok(())
	}

	fn poll_stream(
		&mut self,
		quinn: &mut quinn_proto::Connection,
		event: quinn_proto::StreamEvent,
	) -> Result<(), Error> {
		match event {
			quinn_proto::StreamEvent::Available { dir } => self.open(quinn, dir),
			quinn_proto::StreamEvent::Opened { dir } => self.accept(quinn, dir),
			quinn_proto::StreamEvent::Readable { id } => {}
			quinn_proto::StreamEvent::Writable { id } => {
				let mut quinn = quinn.send_stream(id);

				if let hash_map::Entry::Occupied(mut entry) = self.buffered.entry(id) {
					let written = quinn.write_chunks(&mut [entry.get().clone()])?;
					match written.chunks {
						0 => {
							let _ = entry.get_mut().split_to(written.bytes);
							return Ok(());
						}
						1 => entry.remove(),
						_ => unreachable!(),
					};
				}

				self.encode(quinn, id)?;
			}
			quinn_proto::StreamEvent::Finished { id } => {
				todo!()
			}
			quinn_proto::StreamEvent::Stopped { id, error_code } => {
				todo!()
			}
		};

		Ok(())
	}

	fn accept(&mut self, quinn: &mut quinn_proto::Connection, dir: quinn_proto::Dir) {
		while let Some(id) = quinn.streams().accept(dir) {
			self.generic.streams().accept(dir.into(), id.into());
		}
	}

	fn open(&mut self, quinn: &mut quinn_proto::Connection, dir: quinn_proto::Dir) {
		while let Some(id) = quinn.streams().open(dir) {
			let mut streams = self.generic.streams();

			let mut stream = match streams.open(dir.into(), id.into()) {
				Some(stream) => stream,
				None => continue,
			};

			stream.encode(&mut self.buffer);
		}
	}

	fn decode(&mut self, quinn: &mut quinn_proto::Connection, id: quinn_proto::StreamId) -> Result<(), Error> {
		let mut stream = quinn.recv_stream(id);
		let mut chunks = stream.read(true)?;

		while let Some(chunk) = chunks.next(usize::MAX)? {
			let mut streams = self.generic.streams();
			let mut stream = streams.get(id.into()).unwrap();
			stream.decode(&chunk.bytes)?;
		}

		Ok(())
	}

	fn encode(&mut self, quinn: &mut quinn_proto::SendStream, id: quinn_proto::StreamId) -> Result<(), Error> {
		let mut streams = self.generic.streams();
		let mut stream = streams.get(id.into()).unwrap();

		let mut buffer = std::mem::take(&mut self.buffer);
		stream.encode(&mut buffer);

		let mut buffer = buffer.freeze();

		let written = quinn.write_chunks(&mut [buffer.clone()])?;
		match written.chunks {
			0 => {
				let buffered = buffer.split_off(written.bytes);
				self.buffered.insert(id, buffered);
			}
			1 => {
				// Reuse the buffer if we can.
				self.buffer = buffer.try_into_mut().unwrap_or_default();
			}
			_ => unreachable!(),
		};

		Ok(())
	}
}

impl From<quinn_proto::Dir> for generic::StreamDirection {
	fn from(dir: quinn_proto::Dir) -> Self {
		match dir {
			quinn_proto::Dir::Bi => generic::StreamDirection::Bi,
			quinn_proto::Dir::Uni => generic::StreamDirection::Uni,
		}
	}
}

impl From<generic::StreamDirection> for quinn_proto::Dir {
	fn from(dir: generic::StreamDirection) -> Self {
		match dir {
			generic::StreamDirection::Bi => quinn_proto::Dir::Bi,
			generic::StreamDirection::Uni => quinn_proto::Dir::Uni,
		}
	}
}

impl From<quinn_proto::StreamId> for generic::StreamId {
	fn from(id: quinn_proto::StreamId) -> Self {
		Self(id.0)
	}
}

impl From<generic::StreamId> for quinn_proto::StreamId {
	fn from(id: generic::StreamId) -> Self {
		Self(id.0)
	}
}
