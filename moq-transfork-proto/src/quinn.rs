use std::collections::{hash_map, HashMap};

use bytes::{Bytes, BytesMut};

use crate::generic;

pub struct Connection {
	generic: generic::Connection,
	quinn: quinn_proto::Connection,

	buffer: BytesMut,
	buffered: HashMap<quinn_proto::StreamId, Bytes>,

	stream_uni: Option<generic::StreamId>,
	stream_bi: Option<generic::StreamId>,
}

impl Connection {
	pub fn new(generic: generic::Connection, mut quinn: quinn_proto::Connection) -> Self {
		let stream_uni = quinn.streams().open(quinn_proto::Dir::Uni).map(Into::into);
		let stream_bi = quinn.streams().open(quinn_proto::Dir::Bi).map(Into::into);

		Self {
			generic,
			quinn,
			buffer: BytesMut::new(),
			buffered: HashMap::new(),
			stream_uni,
			stream_bi,
		}
	}

	pub fn poll(&mut self) -> Result<(), Error> {
		while let Some(event) = self.quinn.poll() {
			match event {
				quinn_proto::Event::Stream(event) => self.poll_stream(event)?,
				_ => todo!(),
			}
		}

		Ok(())
	}

	fn poll_stream(&mut self, event: quinn_proto::StreamEvent) -> Result<(), Error> {
		match event {
			quinn_proto::StreamEvent::Opened { dir } => {
				todo!()
			}
			quinn_proto::StreamEvent::Readable { id } => {
				let chunks = self.quinn.recv_stream(id).read(true)?;
				while let Some(chunk) = chunks.next(usize::MAX)? {
					self.generic.decode(id.into(), &chunk.bytes)?;
				}
			}
			quinn_proto::StreamEvent::Writable { id } => {
				let quinn = self.quinn.send_stream(id);
				if let hash_map::Entry::Occupied(entry) = self.buffered.entry(id) {
					let written = quinn.write_chunks(&mut [entry.get().clone()])?;
					match written.chunks {
						0 => {
							entry.get_mut().split_to(written.bytes);
							return Ok(());
						}
						1 => entry.remove(),
						_ => unreachable!(),
					};
				}

				self.generic.encode(id.into(), &mut self.buffer)?;
				let mut buffer = self.buffer.freeze();
				let written = quinn.write_chunks(&mut [buffer.clone()])?;
				match written.chunks {
					0 => {
						let buffered = buffer.split_off(written.bytes);
						self.buffered.insert(id, buffered);
					}
					_ => unreachable!(),
				};

				// Reuse the buffer if we can.
				self.buffer = buffer.try_into_mut().unwrap_or_default();
			}
			quinn_proto::StreamEvent::Finished { id } => {
				todo!()
			}
			quinn_proto::StreamEvent::Stopped { id, error_code } => {
				todo!()
			}
			quinn_proto::StreamEvent::Available { dir } => {
				todo!()
			}
		};

		Ok(())
	}

	fn open_streams(&mut self) {
		loop {
			match self.generic.open_bi(&mut self.stream_bi) {}
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
