use std::collections::HashMap;

use super::{AnnounceId, Connection, Error, GroupId, SubscribeId, SubscribedId};

#[derive(Default)]
pub struct Publisher {
	announce: HashMap<AnnounceId, Announce>,
	subscribed: HashMap<SubscribedId, Subscribed>,
}

impl Publisher {
	pub fn announce(&mut self, path: &str) {}
	pub fn unannounce(&mut self, path: &str) {}

	// Returns a new subscription received.
	pub fn subscribed(&mut self, id: SubscribeId) -> Option<Subscribed<'_>> {}
	pub fn subscribed_next(&mut self) -> Option<Subscribed<'_>> {}
}

pub struct Subscribed<'a> {
	pub id: SubscribeId,
	publisher: &'a mut Publisher,
}

impl<'a> Subscribed<'a> {
	/// Terminate a subscription.
	pub fn cancel(&mut self, error: Error) {}
	pub fn done(mut self) {}

	pub fn group(&mut self, id: GroupId) -> Option<Group<'a>> {}
	pub fn group_next(&mut self) -> Option<Group<'a>> {}
}

pub struct Group<'a> {
	pub id: GroupId,
	subscribed: &'a mut Subscribed<'a>,
}

impl<'a> Group<'a> {
	pub fn cancel(mut self, error: Error) {}
	pub fn done(mut self) {}

	pub fn frame(&mut self, data: &[u8]) {}
	pub fn frame_create(&mut self, size: usize) -> Frame<'a> {}
	pub fn frame_current(&mut self) -> Option<Frame<'a>> {}
}

pub struct Frame<'a> {
	pub size: usize,
	group: &'a mut Group<'a>,
}

impl<'a> Frame<'a> {
	pub fn cancel(mut self, error: Error) {}
	pub fn done(mut self) {}

	pub fn chunk(&mut self, data: Bytes) {}
}

#[derive(Default)]
enum Announce {
	#[default]
	Init,
	Active(message::Announce),
	Closed(Error),
}
