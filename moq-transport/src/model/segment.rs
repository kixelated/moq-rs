//! A segment is a stream of bytes with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of bytes in chunks.
//! There's no framing, so these chunks can be of any size or position, and won't be maintained over the network.
//!
//! A [Subscriber] reads an ordered stream of bytes in chunks.
//! These chunks are returned directly from the QUIC connection, so they may be of any size or position.
//! A closed [Subscriber] will receive a copy of all future chunks. (fanout)
//!
//! The segment is closed with [Error::Closed] when all publishers or subscribers are dropped.
use core::fmt;
use std::{
	future::poll_fn,
	io,
	ops::Deref,
	pin::Pin,
	sync::Arc,
	task::{ready, Context, Poll},
	time,
};

use crate::{Error, VarInt};
use bytes::{Bytes, BytesMut};

use super::Watch;

/// Create a new segment with the given info.
pub fn new(info: Info) -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());
	let info = Arc::new(info);

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

/// Static information about the segment.
#[derive(Debug)]
pub struct Info {
	// The sequence number of the segment within the track.
	pub sequence: VarInt,

	// The priority of the segment within the BROADCAST.
	pub priority: i32,

	// Cache the segment for at most this long.
	pub expires: Option<time::Duration>,
}

struct State {
	// The data that has been received thus far.
	data: Vec<Bytes>,

	// Set when the publisher is dropped.
	closed: Result<(), Error>,
}

impl State {
	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			data: Vec::new(),
			closed: Ok(()),
		}
	}
}

impl fmt::Debug for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		// We don't want to print out the contents, so summarize.
		let size = self.data.iter().map(|chunk| chunk.len()).sum::<usize>();
		let data = format!("size={} chunks={}", size, self.data.len());

		f.debug_struct("State")
			.field("data", &data)
			.field("closed", &self.closed)
			.finish()
	}
}

/// Used to write data to a segment and notify subscribers.
pub struct Publisher {
	// Mutable segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<Info>,

	// Closes the segment when all Publishers are dropped.
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	/// Write a new chunk of bytes.
	pub fn write_chunk(&mut self, data: Bytes) -> Result<(), Error> {
		let mut state = self.state.lock_mut();
		state.closed?;
		state.data.push(data);
		Ok(())
	}

	/// Close the segment with an error.
	pub fn close(self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for Publisher {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Publisher")
			.field("state", &self.state)
			.field("info", &self.info)
			.finish()
	}
}

/// Notified when a segment has new data available.
#[derive(Clone)]
pub struct Subscriber {
	// Modify the segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<Info>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	// A temporary buffer when using AsyncRead.
	buffer: BytesMut,

	// Dropped when all Subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));

		Self {
			state,
			info,
			index: 0,
			buffer: BytesMut::new(),
			_dropped,
		}
	}

	/// Check if there is a chunk available.
	pub fn poll_chunk(&mut self, cx: &mut Context<'_>) -> Poll<Result<Option<Bytes>, Error>> {
		// If there's already buffered data, return it.
		if !self.buffer.is_empty() {
			let chunk = self.buffer.split().freeze();
			return Poll::Ready(Ok(Some(chunk)));
		}

		// Grab the lock and check if there's a new chunk available.
		let state = self.state.lock();

		if self.index < state.data.len() {
			// Yep, clone and return it.
			let chunk = state.data[self.index].clone();
			self.index += 1;
			return Poll::Ready(Ok(Some(chunk)));
		}

		// Otherwise we wait until the state changes and try again.
		match state.closed {
			Err(Error::Closed) => return Poll::Ready(Ok(None)),
			Err(err) => return Poll::Ready(Err(err)),
			Ok(()) => state.waker(cx), // Wake us up when the state changes.
		};

		Poll::Pending
	}

	/// Block until the next chunk of bytes is available.
	pub async fn read_chunk(&mut self) -> Result<Option<Bytes>, Error> {
		poll_fn(|cx| self.poll_chunk(cx)).await
	}
}

impl tokio::io::AsyncRead for Subscriber {
	fn poll_read(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		if !self.buffer.is_empty() {
			// Read from the existing buffer
			let size = std::cmp::min(buf.remaining(), self.buffer.len());
			let data = self.buffer.split_to(size).freeze();
			buf.put_slice(&data);

			return Poll::Ready(Ok(()));
		}

		// Check if there's a new chunk available
		let chunk = ready!(self.poll_chunk(cx));
		let chunk = match chunk {
			// We'll read as much of it as we can, and buffer the rest.
			Ok(Some(chunk)) => chunk,

			// No more data.
			Ok(None) => return Poll::Ready(Ok(())),

			// Crudely cast to io::Error
			Err(err) => return Poll::Ready(Err(err.as_io())),
		};

		// Determine how much of the chunk we can return vs buffer.
		let size = std::cmp::min(buf.remaining(), chunk.len());

		// Return this much.
		buf.put_slice(chunk[..size].as_ref());

		// Buffer this much.
		self.buffer.extend_from_slice(chunk[size..].as_ref());

		Poll::Ready(Ok(()))
	}
}

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl fmt::Debug for Subscriber {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Subscriber")
			.field("state", &self.state)
			.field("info", &self.info)
			.field("index", &self.index)
			.finish()
	}
}

struct Dropped {
	// Modify the segment state.
	state: Watch<State>,
}

impl Dropped {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}
}

impl Drop for Dropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(Error::Closed).ok();
	}
}
