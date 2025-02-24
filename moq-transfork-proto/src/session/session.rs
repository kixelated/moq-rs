use std::collections::{BTreeMap, HashMap, HashSet};

use bytes::{Buf, BufMut};

use crate::message;

use super::{Error, GroupId, StreamCode, StreamId, SubscribeId};

pub struct State {
	streams: Streams,
	publisher: Publisher,
	subscriber: Subscriber,
}

pub struct Streams {}

impl Streams {
	/// Encode the next message for the given stream.
	///
	/// Use `encode_next` to get the next stream, check if flow control allows, then call `encode`.
	pub fn encode<B: BufMut>(&mut self, buf: &mut B, stream: StreamId) {
		unimplemented!()
	}

	/// Return a stream that has (new) data ready to encode.
	pub fn encode_next(&mut self) -> Option<StreamId> {
		unimplemented!()
	}

	/// Decode the next message for the given stream.
	pub fn decode<B: Buf>(&mut self, buf: &mut B, stream: StreamId) {
		unimplemented!()
	}

	/// Close a stream with the given code.
	pub fn close(&mut self, stream: StreamId, code: Option<StreamCode>) {
		unimplemented!()
	}
}

pub struct Subscriber {}

impl Subscriber {
	pub fn announced(&mut self, stream: StreamId, path: Wildcard) {
		todo!()
	}

	pub fn announced_available(&mut self, stream: StreamId) -> Option<WildcardMatch> {
		todo!()
	}

	pub fn announced_unavailable(&mut self, stream: StreamId) -> Option<WildcardMatch> {
		todo!()
	}

	pub fn announced_live(&mut self, stream: StreamId) -> bool {
		todo!()
	}

	pub fn subscribe(&mut self, stream: StreamId, request: SubscribeRequest) {
		todo!()
	}

	pub fn subscribe_update(&mut self, stream: StreamId, update: SubscribeUpdate) -> Result<(), Error> {
		todo!()
	}

	pub fn subscribe_info(&mut self, stream: StreamId) -> Result<SubscribeInfo, Error> {
		todo!()
	}

	/// Returns the next group stream for the given subscription.
	pub fn subscribe_group(&mut self, stream: StreamId) -> Option<StreamId> {
		todo!()
	}

	/// Returns the size of the next frame in the group stream.
	///
	/// The caller is responsible for reading this many bytes from the stream.
	pub fn subscribe_frame(&mut self, stream: StreamId) -> Option<usize> {
		todo!()
	}
}

pub struct Publisher {}

impl Publisher {
	/// Wait until the next announcement is requested.
	pub fn announce_request(&mut self) -> Option<(StreamId, message::AnnouncePlease)> {
		todo!()
	}

	pub fn announce_reply(&mut self, stream: StreamId, response: message::Announce) {
		todo!()
	}

	pub fn subscribe_request(&mut self) -> Option<(StreamId, message::Subscribe)> {
		todo!()
	}

	pub fn subscribe_info(&mut self, stream: StreamId, info: message::Info) {
		todo!()
	}

	pub fn subscribe_group(&mut self, stream: SubscribeId, group: GroupId) {
		todo!()
	}

	pub fn subscribe_frame(&mut self, stream: StreamId, size: usize) {
		todo!()
	}
}

pub struct PublisherStatic {
	active: HashMap<String, PublisherTrack>,
}

impl PublisherStatic {
	pub fn update(&mut self, publisher: &mut Publisher) {
		while let Some((stream, announce)) = publisher.announce_request() {}

		while let Some((stream, subscribe)) = publisher.subscribe_request() {}
	}

	pub fn publish(&mut self, track: String, info: message::Info) {
		todo!()
	}

	pub fn publish_group(&mut self, track: &str, group: GroupId) {
		todo!()
	}

	pub fn publish_frame(&mut self, track: &str, group: GroupId, frame: FrameId) {
		todo!()
	}

	pub fn publish_chunk<B: Buf>(&mut self, track: &str, group: GroupId, frame: FrameId, data: &mut B) {
		todo!()
	}
}

pub struct SubscriberDedup {}

impl SubscriberDedup {
	pub fn update(&mut self, subscriber: &mut Subscriber) {}
}

pub struct Relay {}
