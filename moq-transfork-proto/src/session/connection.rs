use std::collections::{HashMap, VecDeque};

use bytes::Buf;

use crate::{
	coding::{Decode, DecodeError},
	message::{self, ControlType, DataType},
};

use super::Error;

pub struct Connection {
	streams: HashMap<usize, Stream>,

	announce: HashMap<AnnounceId, Announce>,
	announced: HashMap<AnnouncedId, Announced>,
	subscribe: HashMap<SubscirbeId, Subscribe>,
	subscribed: HashMap<SubscribedId, Subscribed>,
}

impl Connection {
	pub fn recv(&mut self, stream: usize, buf: &[u8]) -> Result<(), Error> {
		let stream = self.streams.entry(stream).or_insert_with(Stream::control);
		stream.recv(buf)?;
		Ok(())
	}
}

struct Stream {
	buffer: Option<VecDeque<u8>>,
	kind: StreamKind,
}

impl Stream {
	pub fn control() -> Self {
		Self {
			buffer: Some(VecDeque::new()),
			kind: StreamKind::Control,
		}
	}

	pub fn data() -> Self {
		Self {
			buffer: Some(VecDeque::new()),
			kind: StreamKind::Data,
		}
	}
}

enum StreamKind {
	Control, // Type not yet known
	Data,    // Type not yet known
	Group(Group),
	Session(Session),
	Announce(Announce),
	Subscribe(Subscribe),
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

	fn recv_once<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		match &mut self.kind {
			StreamKind::Control => {
				self.kind = match ControlType::decode(buf)? {
					ControlType::Session => StreamKind::Session(Default::default()),
					ControlType::Announce => StreamKind::Announce(Default::default()),
					ControlType::Subscribe => StreamKind::Subscribe(Default::default()),
					ControlType::Fetch => unimplemented!(),
					ControlType::Info => unimplemented!(),
				}
			}
			StreamKind::Session(state) => {}
			StreamKind::Subscribe(state) => {}
			StreamKind::Announce(state) => {}
			StreamKind::Data => {
				self.kind = match DataType::decode(buf)? {
					DataType::Group => StreamKind::Group(Default::default()),
				}
			}
			StreamKind::Group(state) => {}
		}

		Ok(())
	}
}

#[derive(Default)]
enum Group {
	#[default]
	Init,
	Active(message::Group),
	Closed(Error),
}

#[derive(Default)]
enum Session {
	#[default]
	Init,
	Active,
	Closed(Error),
}

#[derive(Default)]
enum Announce {
	#[default]
	Init,
	Active(message::Announce),
	Closed(Error),
}

#[derive(Default)]
enum Subscribe {
	#[default]
	Init,
	Active(message::Subscribe),
	Closed(Error),
}

impl Subscribe {
	pub fn recv<B: Buf>(&mut self, buf: B) -> Result<(), Error> {
		match self {
			Self::Init => {
				let subscribe = message::Subscribe::decode(buf)?;
				*self = Self::Active(subscribe);
			}
			Self::Active(_) => return Err(Error::Unexpected),
			Self::Closed(_) => return Err(Error::Closed),
		}
	}
}
