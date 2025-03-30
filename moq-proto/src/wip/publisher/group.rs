use std::collections::{HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::Encode,
	message::{self},
	Error, ErrorCode, GroupId, StreamId, SubscribeId,
};

#[derive(Default)]
pub struct PublisherGroups {
	lookup: HashMap<StreamId, PublisherGroup>,
}

impl PublisherGroups {
	pub fn create(
		&mut self,
		stream: StreamId,
		subscribe: SubscribeId,
		group: GroupId,
	) -> Result<&mut PublisherGroup, Error> {
		let state = PublisherGroup::new(stream, subscribe, group);
		let state = self.lookup.entry(stream).or_insert(state);

		Ok(state)
	}

	pub fn get(&mut self, stream: StreamId) -> Option<&mut PublisherGroup> {
		self.lookup.get_mut(&stream)
	}
}

pub struct PublisherGroup {
	id: (SubscribeId, GroupId),
	stream: StreamId,

	frames: VecDeque<usize>,
	chunks: VecDeque<Bytes>,

	write_remain: usize,
	read_remain: usize,
}

impl PublisherGroup {
	fn new(stream: StreamId, subscribe: SubscribeId, group: GroupId) -> Self {
		Self {
			id: (subscribe, group),
			stream,

			frames: VecDeque::new(),
			chunks: VecDeque::new(),

			write_remain: 0,
			read_remain: 0,
		}
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
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

	fn start_frame(&mut self, size: usize) {
		assert_eq!(self.write_remain, 0);
		self.write_remain = size;
		self.frames.push_back(size);
	}

	fn write<B: Buf>(&mut self, buf: &mut B) {
		assert!(self.write_remain > buf.remaining());

		// TODO enforce a maximum buffer size
		let chunk = buf.copy_to_bytes(buf.remaining());
		self.write_remain -= chunk.len();
		self.chunks.push_back(chunk);
	}

	/// Write an entire frame to the stream.
	pub fn write_full<B: Buf>(&mut self, data: &mut B) {
		// NOTE: This does not make a copy if data is already a Bytes.
		let mut data = data.copy_to_bytes(data.remaining());
		self.start_frame(data.len());
		self.write(&mut data);
	}

	/// Mark the start of a new frame with the given size.
	///
	/// WARN: This will panic if the previous frame was not fully written.
	pub fn write_size(&mut self, size: usize) {
		self.start_frame(size);
	}

	/// Write a chunk of the frame, which MUST be preceded by a call to [Self::write_size].
	///
	/// WARN: This will panic if you write more than promised via [Self::write_size].
	pub fn write_chunk<B: Buf>(&mut self, chunk: &mut B) {
		self.write(chunk);
	}

	pub fn close(&mut self, error: Option<ErrorCode>) {
		if let Some(error) = error {
			// TODO
			// self.subscribe.dropped(self.id, error);
		}

		todo!("clean close")
	}
}
