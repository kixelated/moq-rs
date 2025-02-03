use std::{collections::HashMap, fmt, io};

use bytes::{Buf, BufMut, BytesMut};

use crate::{
	coding::{Decode, DecodeError},
	message::{ControlType, DataType},
};

use super::Error;

pub struct Connection {
	streams: HashMap<usize, Stream>,
}

impl Connection {
	pub fn recv<B: bytes::Buf>(&mut self, buf: &mut B, stream: usize) -> Result<(), Error> {
		let stream = self.streams.entry(stream).or_insert_with(Stream::control());
		stream.recv(buf)?;
		Ok(())
	}
}

struct Stream {
	buffer: Option<BytesMut>,
	kind: StreamKind,
}

impl Stream {
	pub fn control() -> Self {
		Self {
			buffer: None,
			kind: StreamKind::Control,
		}
	}

	pub fn data() -> Self {
		Self {
			buffer: None,
			kind: StreamKind::Data,
		}
	}
}

enum StreamKind {
	Control, // Type not yet known
	Data,
	Session,
	Announce,
	Subscribe,
}

impl Stream {
	pub fn recv<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		if let Some(mut buffer) = self.buffer.take() {
			buffer.put(buf);
			let mut cursor = io::Cursor::new(&buffer);
			loop {
				match self.recv_loop(&mut cursor) {
					Ok(()) => {
						buffer.advance(cursor.position() as usize);
					}
					Err(Error::Coding(DecodeError::Short)) => {
						if !buffer.is_empty() {
							self.buffer = Some(buffer);
						}
						return Ok(());
					}
					Err(err) => return Err(err),
				}
			}
		} else {
			let mut cursor = io::Cursor::new(buf);
			loop {
				match self.recv_loop(&mut cursor) {
					Ok(()) => {
						buf.advance(cursor.position() as usize);
					}
					Err(Error::Coding(DecodeError::Short)) => {
						let mut buffer = BytesMut::new();
						buffer.put(buf);
						if !buffer.is_empty() {
							self.buffer = Some(buffer);
						}
						return Ok(());
					}
					Err(err) => return Err(err),
				}
			}
		}
	}

	fn recv_loop<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		match self.kind {
			Self::Init => {
				self.kind = match ControlType::decode(buf)? {
					ControlType::Session => ControlStream::Session,
					ControlType::Announce => ControlStream::Announce,
					ControlType::Subscribe => ControlStream::Subscribe,
					ControlType::Fetch => unimplemented!(),
					ControlType::Info => unimplemented!(),
				}
			}
		}

		Ok(())
	}
}
