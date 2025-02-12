use bytes::Bytes;

use crate::message::{self, Info};

use super::Error;

pub struct AnnounceId(usize);
pub struct SubscribeId(usize);

pub struct GroupId(usize);
pub struct FrameId(usize);

pub enum AnnounceEvent {
	Active(String),
	Ended(String),
	Live,
}

pub enum SubscribeEvent {
	Start(Info),
	Group(GroupId, GroupEvent),
	Final,
	Error(Error),
}

pub enum GroupEvent {
	Start,
	Frame(FrameId, FrameEvent),
	Final,
	Error(Error),
}

pub enum FrameEvent {
	Size(usize),
	Chunk(Bytes),
	Final,
	Error(Error),
}

pub struct Subscriber {}

impl Subscriber {
	/// Return any announced tracks matching the path.
	pub fn announced(&mut self, path: &str) -> AnnounceId {
		todo!();
	}

	/// Return the next announced with at least one event ready.
	pub fn announced_ready(&mut self) -> Option<AnnounceId> {
		todo!();
	}

	/// Return the next announced event.
	pub fn announced_event(&mut self, id: AnnounceId) -> Option<AnnounceEvent> {
		todo!();
	}

	/// Stop receiving announcements.
	pub fn announced_cancel(&mut self, id: AnnounceId) -> Result<(), Error> {
		todo!()
	}

	// Start a subscription, returning a handle to it.
	pub fn subscribe(&mut self, subscribe: message::Subscribe) -> SubscribeId {
		todo!();
	}

	// Returns a SubscribeId that has at least one event ready.
	pub fn subscribe_ready(&mut self) -> Option<SubscribeId> {
		todo!();
	}

	// Returns the next event for a subscription.
	pub fn subscribe_event(&mut self, id: SubscribeId) -> Option<SubscribeEvent> {
		todo!();
	}

	// Stop a subscription.
	pub fn subscribe_stop(&mut self, id: SubscribeId) -> Result<(), Error> {
		todo!()
	}
}
