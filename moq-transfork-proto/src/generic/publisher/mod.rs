mod announce;
mod group;
mod subscribe;

pub use announce::*;
use derive_more::From;
pub use group::*;
pub use subscribe::*;

use bytes::{Buf, BufMut};

use super::{AnnounceId, Error, GroupId, StreamId, StreamsState, SubscribeId};

#[derive(Debug, Clone, Copy, From)]
pub enum PublisherEvent {
	/// An announcement is requested.
	///
	/// Call `announce(id)` with the announcement ID and reply with available tracks.
	Announce(AnnounceId),

	/// A subscription is requested.
	///
	/// Call `subscribe(id)` with the subscription ID and reply to the request.
	Subscribe(SubscribeId),
}

#[derive(Debug, Clone, Copy, From)]
pub(crate) enum PublisherStream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
	Group((SubscribeId, GroupId)),
}

#[derive(Default)]
pub(super) struct PublisherState {
	announces: PublisherAnnouncesState,
	subscribes: PublisherSubscribesState,
	groups: PublisherGroupsState,
}

impl PublisherState {
	pub fn encode<B: BufMut>(&mut self, kind: PublisherStream, buf: &mut B) {
		match kind {
			PublisherStream::Announce(id) => self.announces.encode(id, buf),
			PublisherStream::Subscribe(id) => self.subscribes.encode(id, buf),
			PublisherStream::Group(id) => self.groups.encode(id, buf),
		}
	}

	pub fn decode<B: Buf>(&mut self, kind: PublisherStream, buf: &mut B) -> Result<(), Error> {
		match kind {
			PublisherStream::Announce(id) => self.announces.decode(id, buf),
			PublisherStream::Subscribe(id) => self.subscribes.decode(id, buf),
			PublisherStream::Group(_) => unreachable!("write only"),
		}
	}

	pub fn accept_announce<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<PublisherStream, Error> {
		let id = self.announces.accept(stream, buf)?;
		Ok(PublisherStream::Announce(id))
	}

	pub fn accept_subscribe<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<PublisherStream, Error> {
		let id = self.subscribes.accept(stream, buf)?;
		Ok(PublisherStream::Subscribe(id))
	}

	pub fn open(&mut self, stream: StreamId) -> Option<PublisherStream> {
		self.groups.open(stream).map(Into::into)
	}
}

pub struct Publisher<'a> {
	pub(super) state: &'a mut PublisherState,
	pub(super) streams: &'a mut StreamsState,
}

impl Publisher<'_> {
	pub fn announces(&mut self) -> PublisherAnnounces {
		PublisherAnnounces {
			state: &mut self.state.announces,
			streams: self.streams,
		}
	}

	pub fn subscribes(&mut self) -> PublisherSubscribes {
		PublisherSubscribes {
			state: &mut self.state.subscribes,
			streams: self.streams,
		}
	}
}
