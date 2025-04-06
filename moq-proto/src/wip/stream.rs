use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_more::From;

use crate::{
	coding::{Decode, Encode},
	message,
};

use super::{Error, StreamId};

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub enum StreamDir {
	Uni,
	Bi,
}
pub struct Stream {
	id: StreamId,
	dir: StreamDir,
	kind: StreamKind,

	send: BytesMut,
	recv: Bytes,
}

impl Stream {
	pub fn accept<B: Buf>(id: StreamId, dir: StreamDir, buf: &mut B) -> Result<Self, Error> {
		assert!(buf.has_remaining(), "empty accepts not yet supported");

		let kind = match dir {
			StreamDir::Uni => match message::DataType::decode(buf)? {
				message::DataType::Group => StreamKind::Group,
			},
			StreamDir::Bi => match message::ControlType::decode(buf)? {
				message::ControlType::Announce => StreamKind::Announce,
				message::ControlType::Subscribe => StreamKind::Subscribe,
				message::ControlType::Session => StreamKind::Session,
				message::ControlType::Info => unimplemented!(),
			},
		};

		let recv = buf.copy_to_bytes(buf.remaining());

		Ok(Self {
			id,
			dir,
			kind,
			send: BytesMut::new(),
			recv,
		})
	}

	pub fn kind(&self) -> StreamKind {
		self.kind
	}

	pub fn open(id: StreamId, dir: StreamDir, kind: StreamKind) -> Self {
		let mut send = BytesMut::new();

		match kind {
			StreamKind::Group => {
				assert_eq!(dir, StreamDir::Uni);
				message::DataType::Group.encode(&mut send);
			}
			StreamKind::Session => {
				assert_eq!(dir, StreamDir::Bi);
				message::ControlType::Session.encode(&mut send);
			}
			StreamKind::Announce => {
				assert_eq!(dir, StreamDir::Bi);
				message::ControlType::Announce.encode(&mut send);
			}
			StreamKind::Subscribe => {
				assert_eq!(dir, StreamDir::Bi);
				message::ControlType::Subscribe.encode(&mut send);
			}
		}

		Self {
			id,
			dir,
			kind,
			recv: Bytes::new(),
			send,
		}
	}

	pub fn id(&self) -> StreamId {
		self.id
	}

	pub(crate) fn encode<B: BufMut, E: Encode>(&mut self, b: &mut B, msg: E) {
		if !self.send.is_empty() {
			let size = b.remaining_mut().min(self.send.len());
			let chunk = self.send.split_to(size);
			b.put(chunk);
		}

		let mut buf = b.chain_mut(&mut self.send);
		msg.encode(&mut buf);
	}

	pub(crate) fn try_decode<B: Buf, D: Decode>(&mut self, b: &mut B) -> Result<Option<D>, Error> {
		let recv = std::mem::take(&mut self.recv);

		let mut buf = recv.chain(b);
		let msg = D::decode(&mut buf)?;
		self.recv = buf.copy_to_bytes(buf.remaining());

		Ok(Some(msg))
	}
}

#[derive(Clone, Debug, From, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
	Session,
	Announce,
	Subscribe,
	Group,
}
