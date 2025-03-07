mod announce;
mod group;
mod subscribe;

pub use announce::*;
pub use group::*;
pub use subscribe::*;


use bytes::{Buf, BufMut};
use derive_more::From;

use crate::coding::{Decode, Encode};

use super::{AnnounceId, Error, GroupId, StreamId, StreamsState, SubscribeId};

#[derive(Debug, Clone, Copy, From)]
pub(super) enum SubscriberStream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
	Group((SubscribeId, GroupId)),
}

#[derive(Debug)]
pub enum SubscriberEvent {
	/// An announcement has new data.
	///
	/// Call `announce(id)` with the announcement ID to learn more.
	Announce(AnnounceId),

	/// A subscription has new data.
	///
	/// Call `subscribe(id)` with the subscription ID to learn more.
	Subscribe(SubscribeId),

	/// A group has new data.
	///
	/// Call `group(id)` with the group ID to learn more.
	Group(GroupId),
}

#[derive(Default)]
pub(super) struct SubscriberState {
	announces: SubscriberAnnouncesState,
	subscribes: SubscriberSubscribesState,
	groups: SubscriberGroupsState,
}

impl SubscriberState {
	pub fn encode<B: BufMut>(&mut self, kind: SubscriberStream, buf: &mut B) {
		match kind {
			SubscriberStream::Announce(id) => self.announces.encode(id, buf),
			SubscriberStream::Subscribe(id) => self.subscribes.encode(id, buf),
			SubscriberStream::Group(_) => unreachable!("read only"),
		}
	}

	pub fn decode<B: Buf>(&mut self, kind: SubscriberStream, buf: &mut B) -> Result<(), Error> {
		match kind {
			SubscriberStream::Announce(id) => self.announces.decode(id, buf),
			SubscriberStream::Subscribe(id) => self.subscribes.decode(id, buf),
			SubscriberStream::Group(id) => self.groups.decode(id, buf),
		}
	}

	pub fn open(&mut self, stream: StreamId, kind: SubscriberStream) {
		match kind {
			SubscriberStream::Announce(id) => self.announces.open(id, stream),
			SubscriberStream::Subscribe(id) => self.subscribes.open(id, stream),
			SubscriberStream::Group(_) => unreachable!("publisher opens"),
		}
	}

	pub fn accept_group<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<SubscriberStream, Error> {
		let id = self.groups.accept(stream, buf)?;
		Ok(SubscriberStream::Group(id))
	}
}

pub struct Subscriber<'a> {
	pub(super) state: &'a mut SubscriberState,
	pub(super) streams: &'a mut StreamsState,
}

impl Subscriber<'_> {
	pub fn announces(&mut self) -> SubscriberAnnounces {
		SubscriberAnnounces {
			state: &mut self.state.announces,
			streams: self.streams,
		}
	}

	pub fn subscribes(&mut self) -> SubscriberSubscribes {
		SubscriberSubscribes {
			state: &mut self.state.subscribes,
			streams: self.streams,
		}
	}

	pub fn groups(&mut self) -> SubscriberGroups {
		SubscriberGroups {
			state: &mut self.state.groups,
			streams: self.streams,
		}
	}
}
