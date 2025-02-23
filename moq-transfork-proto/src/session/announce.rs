use std::collections::{hash_map, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::{Decode, Encode},
	message::{self, GroupOrder, Info, Path},
};

use super::{Error, FrameId, GroupId, Stream, StreamCode, StreamId};

pub type AnnounceRequest = message::AnnouncePlease;

pub struct Announced {
	lookup: HashMap<StreamId, AnnounceState>,
}

impl Announced {
	/// Request any tracks matching the specified path.
	pub fn start(&mut self, stream: StreamId, path: &str) -> Result<(), Error> {
		todo!();
	}

	/// Return the next announced event, if any
	pub fn event(&mut self) -> Option<(StreamId, AnnounceEvent)> {
		todo!();
	}

	/// Stop receiving announcements.
	pub fn stop(&mut self, id: StreamId) -> Result<(), Error> {
		todo!()
	}
}

struct AnnounceState {
	request: Option<AnnounceRequest>,
}

impl Announce {
	fn new(request: AnnounceRequest) -> Self {
		Self { request: Some(request) }
	}

	pub fn event(&mut self) -> Option<AnnounceRequest> {
		todo!();
	}
}

impl Stream for Announce {
	fn encode<B: BufMut>(&mut self, b: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(b);
		}
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let _drop = message::GroupDrop::decode(buf)?;
		// TODO use
		Ok(())
	}

	fn closed(&mut self) -> Option<u8> {
		todo!();
	}

	fn close(&mut self, code: u8) {
		todo!();
	}
}
