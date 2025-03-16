mod announce;
mod group;
mod subscribe;

use std::collections::BTreeSet;

pub use announce::*;
use derive_more::From;
pub use group::*;
pub use subscribe::*;

use bytes::{Buf, BufMut};

use super::{AnnounceId, Error, GroupId, StreamId, StreamsState, SubscribeId};

#[derive(Debug, From)]
pub enum PublisherEvent {
	/// An announcement is requested.
	Announce(AnnounceId),

	/// A subscription is requested.
	Subscribe(SubscribeId),
}

#[derive(Debug, Clone, Copy, From)]
pub(crate) enum PublisherStream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
	Group((SubscribeId, GroupId)),
}

#[derive(Default)]
pub(super) struct Publisher {
	announces: PublisherAnnounces,
	announces_ready: BTreeSet<AnnounceId>,

	subscribes: PublisherSubscribes,
	subscribes_ready: BTreeSet<SubscribeId>,

	groups: PublisherGroups,
}

impl Publisher {
	pub(crate) fn encode<B: BufMut>(&mut self, kind: PublisherStream, buf: &mut B) {
		match kind {
			PublisherStream::Announce(id) => self.announces.encode(id, buf),
			PublisherStream::Subscribe(id) => self.subscribes.encode(id, buf),
			PublisherStream::Group(id) => self.groups.encode(id, buf),
		}
	}

	pub(crate) fn decode<B: Buf>(&mut self, kind: PublisherStream, buf: &mut B) -> Result<(), Error> {
		match kind {
			PublisherStream::Announce(id) => self.announces.decode(id, buf),
			PublisherStream::Subscribe(id) => self.subscribes.decode(id, buf),
			PublisherStream::Group(_) => unreachable!("write only"),
		}
	}

	pub(crate) fn accept_announce<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<PublisherStream, Error> {
		let id = self.announces.accept(stream, buf)?;
		self.announces_ready.insert(id);
		Ok(PublisherStream::Announce(id))
	}

	pub(crate) fn accept_subscribe<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<PublisherStream, Error> {
		let id = self.subscribes.accept(stream, buf)?;
		self.subscribes_ready.insert(id);
		Ok(PublisherStream::Subscribe(id))
	}

	pub(crate) fn open(&mut self, stream: StreamId) -> Option<PublisherStream> {
		self.groups.open(stream).map(Into::into)
	}

	pub fn poll(&mut self) -> Option<PublisherEvent> {
		if let Some(id) = self.announces_ready.pop_first() {
			Some(PublisherEvent::Announce(id))
		} else if let Some(id) = self.subscribes_ready.pop_first() {
			Some(PublisherEvent::Subscribe(id))
		} else {
			None
		}
	}

	pub fn announces(&mut self) -> PublisherAnnounces {
		&mut self.announces
	}

	pub fn subscribes(&mut self) -> PublisherSubscribes {
		&mut self.subscribes
	}
}
