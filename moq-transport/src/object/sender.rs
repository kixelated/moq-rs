use std::sync::{Mutex, Weak};
use std::task::{self, Poll};
use std::{collections::BinaryHeap, sync::Arc};

use anyhow::Context;
use bytes::{Buf, BytesMut};

use crate::coding::Encode;
use crate::object::Header;

use webtransport_generic::SendStream as GenericSendStream;
use webtransport_generic::{AsyncSendStream, AsyncSession};

// Allow this to be cloned so we can have multiple senders.
pub struct Sender<S>
where
	S: AsyncSession,
	S::SendStream: AsyncSendStream,
{
	// The session.
	session: S,

	// A reusable buffer for the stream header.
	buf: BytesMut,

	// Register new streams with an inner object that will prioritize them.
	inner: Arc<Mutex<SenderInner<S::SendStream>>>,
}

impl<S> Sender<S>
where
	S: AsyncSession,
	S::SendStream: AsyncSendStream,
{
	pub fn new(session: S) -> Self {
		let inner = SenderInner::new();
		Self {
			session,
			buf: BytesMut::new(),
			inner: Arc::new(Mutex::new(inner)),
		}
	}

	pub async fn open(&mut self, header: Header) -> anyhow::Result<SendStream<S::SendStream>> {
		let stream = self.session.open_uni().await.context("failed to open uni stream")?;

		let mut stream = {
			let mut inner = self.inner.lock().unwrap();
			inner.register(stream, header.send_order.into_inner())?
		};

		self.buf.clear();
		header.encode(&mut self.buf).unwrap();
		stream.send_all(&mut self.buf).await.context("failed to write header")?;

		// log::info!("created stream: {:?}", header);

		header.encode(&mut self.buf).unwrap();
		stream.send_all(&mut self.buf).await.context("failed to write header")?;

		Ok(stream)
	}
}

impl<S> Clone for Sender<S>
where
	S: AsyncSession,
	S::SendStream: AsyncSendStream,
{
	fn clone(&self) -> Self {
		Sender {
			session: self.session.clone(),
			buf: BytesMut::new(),
			inner: self.inner.clone(),
		}
	}
}

struct SenderInner<S>
where
	S: GenericSendStream,
{
	// Quinn supports a i32 for priority, but the wire format is a u64.
	// Our work around is to keep a list of streams in priority order and use the index as the priority.
	// This involves more work, so TODO either increase the Quinn size or reduce the wire size.
	ordered: BinaryHeap<SendOrder<S>>,
	ordered_swap: BinaryHeap<SendOrder<S>>, // reuse memory to avoid allocations
}

impl<S> SenderInner<S>
where
	S: GenericSendStream,
{
	fn new() -> Self {
		Self {
			ordered: BinaryHeap::new(),
			ordered_swap: BinaryHeap::new(),
		}
	}

	pub fn register(&mut self, stream: S, order: u64) -> anyhow::Result<SendStream<S>> {
		let stream = SendStream::new(stream);
		let order = SendOrder::new(&stream, order);

		// Add the priority to our existing list.
		self.ordered.push(order);

		// Loop through the list and update the priorities of any still active streams.
		let mut index = 0;
		while let Some(stream) = self.ordered.pop() {
			if stream.set_priority(index).is_some() {
				// Add the stream to the new list so it'll be in sorted order.
				self.ordered_swap.push(stream);
				index += 1;
			}
		}

		// Swap the lists so we can reuse the memory.
		std::mem::swap(&mut self.ordered, &mut self.ordered_swap);

		Ok(stream)
	}
}

struct SendOrder<S>
where
	S: GenericSendStream,
{
	// We use Weak here so we don't prevent the stream from being closed when dereferenced.
	// set_priority() will return None if the stream was closed.
	stream: Weak<Mutex<S>>,
	order: u64,
}

impl<S> SendOrder<S>
where
	S: GenericSendStream,
{
	fn new(stream: &SendStream<S>, order: u64) -> Self {
		let stream = stream.weak();
		Self { stream, order }
	}

	fn set_priority(&self, index: i32) -> Option<()> {
		let stream = self.stream.upgrade()?;
		let mut stream = stream.lock().unwrap();
		stream.set_priority(index);
		Some(())
	}
}

impl<S> PartialEq for SendOrder<S>
where
	S: GenericSendStream,
{
	fn eq(&self, other: &Self) -> bool {
		self.order == other.order
	}
}

impl<S> Eq for SendOrder<S> where S: GenericSendStream {}

impl<S> PartialOrd for SendOrder<S>
where
	S: GenericSendStream,
{
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		// We reverse the order so the lower send order is higher priority.
		other.order.partial_cmp(&self.order)
	}
}

impl<S> Ord for SendOrder<S>
where
	S: GenericSendStream,
{
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		// We reverse the order so the lower send order is higher priority.
		other.order.cmp(&self.order)
	}
}

// Ugh, so we need to wrap SendStream with a mutex because we need to be able to call set_priority on it.
// The problem is that set_priority takes a i32, while send_order is a VarInt
// So the solution is to maintain a priority queue of active streams and constantly update the priority with their index.
// So the library might update the priority of the stream at any point, while the application might similtaniously write to it.
pub struct SendStream<S>
where
	S: GenericSendStream,
{
	// All SendStream methods are &mut, so we need to wrap them with an internal mutex.
	inner: Arc<Mutex<S>>,
}

impl<S> SendStream<S>
where
	S: GenericSendStream,
{
	pub(crate) fn new(stream: S) -> Self {
		Self {
			inner: Arc::new(Mutex::new(stream)),
		}
	}

	pub fn weak(&self) -> Weak<Mutex<S>> {
		Arc::<Mutex<S>>::downgrade(&self.inner)
	}
}

impl<S> GenericSendStream for SendStream<S>
where
	S: GenericSendStream,
{
	type Error = S::Error;

	fn poll_send<B: Buf>(&mut self, cx: &mut task::Context<'_>, buf: &mut B) -> Poll<Result<usize, Self::Error>> {
		self.inner.lock().unwrap().poll_send(cx, buf)
	}

	fn reset(&mut self, reset_code: u32) {
		self.inner.lock().unwrap().reset(reset_code)
	}

	// The application should NOT use this method.
	// The library will automatically set the stream priority on creation based on the header.
	fn set_priority(&mut self, order: i32) {
		self.inner.lock().unwrap().set_priority(order)
	}
}
