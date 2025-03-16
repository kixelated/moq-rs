use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::Encode,
	message::{self},
	Error, ErrorCode, GroupId, StreamId, StreamsState, SubscribeId,
};

use super::PublisherStream;

#[derive(Default)]
pub struct PublisherGroups {
	lookup: HashMap<(SubscribeId, GroupId), PublisherGroup>,

	// The groups that are waiting for a stream to be opened.
	open: BTreeSet<(SubscribeId, GroupId)>, // TODO sort by priority
}

impl PublisherGroups {
	pub(crate) fn encode<B: BufMut>(&mut self, id: (SubscribeId, GroupId), buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}

	pub(crate) fn open(&mut self, stream: StreamId) -> Option<PublisherStream> {
		if let Some(id) = self.open.pop_first() {
			self.lookup.get_mut(&id).unwrap().open(stream);
			Some(id.into())
		} else {
			None
		}
	}

	pub fn create(&mut self, subscribe: SubscribeId, group: GroupId) -> Result<&mut PublisherGroup, Error> {
		let id = (subscribe, group);

		self.streams.open(PublisherStream::Group(id).into());
		self.open.insert(id);

		let state = PublisherGroup::default();
		let state = self.lookup.entry(id).or_insert(state);

		Ok(state)
	}

	pub fn get(&mut self, subscribe: SubscribeId, group: GroupId) -> Option<&mut PublisherGroup> {
		let id = (subscribe, group);
		self.lookup.get_mut(&id)
	}
}

#[derive(Default)]
pub(super) struct PublisherGroup {
	id: (SubscribeId, GroupId),

	stream: Option<StreamId>,

	frames: VecDeque<usize>,
	chunks: VecDeque<Bytes>,

	write_remain: usize,
	read_remain: usize,
}

impl PublisherGroup {
	pub(crate) fn open(&mut self, stream: StreamId) {
		assert!(self.stream.is_none());
		self.stream = Some(stream);
	}

	pub(crate) fn frame(&mut self, size: usize) {
		assert_eq!(self.write_remain, 0);
		self.write_remain = size;
		self.frames.push_back(size);
	}

	pub(crate) fn write<B: Buf>(&mut self, buf: &mut B) {
		assert!(self.write_remain > buf.remaining());

		// TODO enforce a maximum buffer size
		let chunk = buf.copy_to_bytes(buf.remaining());
		self.write_remain -= chunk.len();
		self.chunks.push_back(chunk);
	}

	pub(crate) fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if self.read_remain == 0 {
			let size = match self.frames.pop_front() {
				Some(size) => size,
				None => return,
			};

			self.read_remain = size;

			message::Frame { size }.encode(buf);
		}

		if let Some(mut chunk) = self.chunks.pop_front() {
			let size = buf.remaining_mut().min(self.read_remain);
			buf.limit(size).put(&mut chunk);

			self.read_remain -= size;

			if chunk.len() > size {
				self.chunks.push_front(chunk);
			}
		}
	}

	pub fn id(&self) -> (SubscribeId, GroupId) {
		self.id
	}

	/// Write an entire frame to the stream.
	pub fn write_full<B: Buf>(&mut self, data: &mut B) {
		// NOTE: This does not make a copy if data is already a Bytes.
		let mut data = data.copy_to_bytes(data.remaining());
		self.frame(data.len());
		self.write(&mut data);

		if let Some(stream) = self.stream {
			self.streams.encodable(stream);
		}
	}

	/// Mark the start of a new frame with the given size.
	///
	/// WARN: This will panic if the previous frame was not fully written.
	pub fn write_size(&mut self, size: usize) {
		self.frame(size);
	}

	/// Write a chunk of the frame, which MUST be preceded by a call to [Self::write_size].
	///
	/// WARN: This will panic if you write more than promised via [Self::write_size].
	pub fn write_chunk<B: Buf>(&mut self, chunk: &mut B) {
		self.write(chunk);

		if let Some(stream) = self.stream {
			self.streams.encodable(stream);
		}
	}

	pub fn close(&mut self, error: Option<ErrorCode>) {
		if let Some(error) = error {
			// TODO
			// self.subscribe.dropped(self.id, error);
		}

		if let Some(stream) = self.stream {
			self.streams.encodable(stream);
		}

		todo!("clean close")
	}
}
