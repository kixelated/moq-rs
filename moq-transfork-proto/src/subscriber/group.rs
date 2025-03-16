use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::Decode,
	message::{self},
	Error, GroupId, StreamId, SubscribeId,
};

#[derive(Default)]
pub struct SubscriberGroups {
	lookup: HashMap<(SubscribeId, GroupId), SubscriberGroup>,
	ready: BTreeSet<(SubscribeId, GroupId)>,
}

impl SubscriberGroups {
	pub(crate) fn decode<B: Buf>(&mut self, id: (SubscribeId, GroupId), buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)
	}

	pub(crate) fn accept<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<(SubscribeId, GroupId), Error> {
		let msg = message::Group::decode(buf)?;
		let id = (msg.subscribe.into(), msg.sequence.into());

		let state = SubscriberGroup::new(stream);
		self.lookup.insert(id, state);
		self.ready.insert(id);

		Ok(id)
	}
}

pub struct SubscriberGroup {
	stream: StreamId,

	frames: VecDeque<usize>,
	data: VecDeque<u8>,

	write_remain: usize,
	read_remain: usize,
}

impl SubscriberGroup {
	pub(crate) fn new(stream: StreamId) -> Self {
		Self {
			stream,
			frames: VecDeque::new(),
			data: VecDeque::new(),
			write_remain: 0,
			read_remain: 0,
		}
	}

	/// Decode the next frame from the stream.
	pub(crate) fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		if self.write_remain == 0 {
			let frame = message::Frame::decode(buf)?;
			self.write_remain = frame.size;
		}

		let size = buf.remaining().min(self.write_remain);
		let mut buf = buf.take(size);
		while buf.has_remaining() {
			let chunk = buf.chunk();
			self.data.extend(chunk);
			buf.advance(chunk.len());
		}

		Ok(())
	}

	/// Read the next chunk of the frame if available, returning the remaining size until the next frame.
	pub(crate) fn read<B: BufMut>(&mut self, buf: &mut B) -> Option<usize> {
		if self.read_remain == 0 {
			match self.frames.pop_front() {
				Some(size) => self.read_remain = size,
				None => return None,
			}
		}

		let size = buf.remaining_mut().min(self.read_remain);
		buf.limit(size).put(&mut self.data);

		self.read_remain -= size;
		Some(self.read_remain)
	}
}
