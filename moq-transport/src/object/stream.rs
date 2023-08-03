use std::{
	sync::{Arc, Mutex, Weak},
	task::{Context, Poll},
};

use bytes::{Buf, BufMut, Bytes};

use webtransport_generic::RecvStream as GenericRecvStream;
use webtransport_generic::SendStream as GenericSendStream;

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

	fn poll_send<B: Buf>(&mut self, cx: &mut Context<'_>, buf: &mut B) -> Poll<Result<usize, Self::Error>> {
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

// Unfortunately, we need to wrap RecvStream with a buffer since moq-transport::Coding only supports buffered reads.
// We first serve any data in the buffer, then we poll the stream.
// TODO fix this so we don't need the wrapper.
pub struct RecvStream<R>
where
	R: GenericRecvStream,
{
	buf: Bytes,
	stream: R,
}

impl<R> RecvStream<R>
where
	R: GenericRecvStream,
{
	pub(crate) fn new(buf: Bytes, stream: R) -> Self {
		Self { buf, stream }
	}

	pub fn stop(&mut self, code: u32) {
		self.stream.stop(code)
	}
}

impl<R> GenericRecvStream for RecvStream<R>
where
	R: GenericRecvStream,
{
	type Error = R::Error;

	fn poll_recv<B: BufMut>(&mut self, cx: &mut Context<'_>, buf: &mut B) -> Poll<Result<Option<usize>, Self::Error>> {
		if !self.buf.is_empty() {
			let size = self.buf.len();
			buf.put(&mut self.buf);
			let size = size - self.buf.len();
			Poll::Ready(Ok(Some(size)))
		} else {
			self.stream.poll_recv(cx, buf)
		}
	}

	fn stop(&mut self, error_code: u32) {
		self.stream.stop(error_code)
	}
}
