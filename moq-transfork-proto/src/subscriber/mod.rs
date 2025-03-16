mod announce;
mod group;
mod subscribe;

pub use announce::*;
pub use group::*;
pub use subscribe::*;

use bytes::{Buf, BufMut};
use derive_more::From;

use super::{AnnounceId, Error, GroupId, StreamId, SubscribeId};

#[derive(Debug, Clone, Copy, From)]
pub(crate) enum SubscriberStream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
	Group((SubscribeId, GroupId)),
}

#[derive(Debug)]
pub enum SubscriberEvent {
	/// An announcement has new data.
	///
	/// Call `announce(id).poll()` to learn more.
	Announce(AnnounceId),

	/// A subscription has new data.
	///
	/// Call `subscribe(id).poll()` to learn more.
	Subscribe(SubscribeId),

	/// A group has new data.
	///
	/// Call `group(id).poll()` to learn more.
	Group(GroupId),
}

#[derive(Default)]
pub struct Subscriber {
	announces: SubscriberAnnounces,
	subscribes: SubscriberSubscribes,
	groups: SubscriberGroups,
}

impl Subscriber {
	pub(crate) fn encode<B: BufMut>(&mut self, kind: SubscriberStream, buf: &mut B) {
		match kind {
			SubscriberStream::Announce(id) => self.announces.encode(id, buf),
			SubscriberStream::Subscribe(id) => self.subscribes.encode(id, buf),
			SubscriberStream::Group(_) => unreachable!("read only"),
		}
	}

	pub(crate) fn decode<B: Buf>(&mut self, kind: SubscriberStream, buf: &mut B) -> Result<(), Error> {
		match kind {
			SubscriberStream::Announce(id) => self.announces.decode(id, buf),
			SubscriberStream::Subscribe(id) => self.subscribes.decode(id, buf),
			SubscriberStream::Group(id) => self.groups.decode(id, buf),
		}
	}

	pub(crate) fn open(&mut self, stream: StreamId) -> Option<SubscriberStream> {
		self.announces.open(stream).or_else(|| self.subscribes.open(stream))
	}

	pub(crate) fn accept_group<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<SubscriberStream, Error> {
		let id = self.groups.accept(stream, buf)?;
		Ok(SubscriberStream::Group(id))
	}

	pub fn announces(&mut self) -> &mut SubscriberAnnounces {
		&mut self.announces
	}

	pub fn subscribes(&mut self) -> &mut SubscriberSubscribes {
		&mut self.subscribes
	}

	pub fn groups(&mut self) -> &mut SubscriberGroups {
		&mut self.groups
	}
}
