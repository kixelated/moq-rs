use std::{
	io,
	pin::{pin, Pin},
	sync::{Arc, Mutex, Weak},
	task::{self, Poll},
};

use bytes::{BufMut, Bytes};
use tokio::io::{AsyncRead, AsyncWrite};

// Ugh, so we need to wrap SendStream with a mutex because we need to be able to call set_priority on it.
// The problem is that set_priority takes a i32, while send_order is a VarInt
// So the solution is to maintain a priority queue of active streams and constantly update the priority with their index.
// So the library might update the priority of the stream at any point, while the application might similtaniously write to it.
// The only upside is that we don't expose set_priority, so the application can't screw with things.
pub struct SendStream {
	stream: Arc<Mutex<webtransport_quinn::SendStream>>,
}

impl SendStream {
	// Create a new stream with the given order, returning a handle that allows us to update the priority.
	pub(crate) fn with_order(stream: webtransport_quinn::SendStream, order: u64) -> (Self, SendStreamOrder) {
		let stream = Arc::new(Mutex::new(stream));
		let weak = Arc::<Mutex<webtransport_quinn::SendStream>>::downgrade(&stream);

		(SendStream { stream }, SendStreamOrder { stream: weak, order })
	}
}

pub(crate) struct SendStreamOrder {
	// We use Weak here so we don't prevent the stream from being closed when dereferenced.
	// update() will return an error if the stream was closed instead.
	stream: Weak<Mutex<webtransport_quinn::SendStream>>,
	order: u64,
}

impl SendStreamOrder {
	pub(crate) fn update(&self, index: i32) -> Result<(), webtransport_quinn::StreamClosed> {
		let stream = self.stream.upgrade().ok_or(webtransport_quinn::StreamClosed)?;
		let mut stream = stream.lock().unwrap();
		stream.set_priority(index)
	}
}

impl PartialEq for SendStreamOrder {
	fn eq(&self, other: &Self) -> bool {
		self.order == other.order
	}
}

impl Eq for SendStreamOrder {}

impl PartialOrd for SendStreamOrder {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		// We reverse the order so the lower send order is higher priority.
		other.order.partial_cmp(&self.order)
	}
}

impl Ord for SendStreamOrder {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		// We reverse the order so the lower send order is higher priority.
		other.order.cmp(&self.order)
	}
}

// We implement AsyncWrite so we can grab the mutex on each write attempt, instead of holding it for the entire async function.
impl AsyncWrite for SendStream {
	fn poll_write(self: Pin<&mut Self>, cx: &mut task::Context<'_>, buf: &[u8]) -> task::Poll<io::Result<usize>> {
		let mut stream = self.stream.lock().unwrap();
		Pin::new(&mut *stream).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
		let mut stream = self.stream.lock().unwrap();
		Pin::new(&mut *stream).poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<io::Result<()>> {
		let mut stream = self.stream.lock().unwrap();
		Pin::new(&mut *stream).poll_shutdown(cx)
	}
}

// Unfortunately, we need to wrap RecvStream with a buffer since moq-transport::Coding only supports buffered reads.
// We first serve any data in the buffer, then we poll the stream.
pub struct RecvStream {
	buf: Bytes,
	stream: webtransport_quinn::RecvStream,
}

impl RecvStream {
	pub(crate) fn new(buf: Bytes, stream: webtransport_quinn::RecvStream) -> Self {
		Self { buf, stream }
	}

	pub fn stop(&mut self, code: u32) {
		self.stream.stop(code).ok();
	}
}

impl AsyncRead for RecvStream {
	fn poll_read(
		mut self: Pin<&mut Self>,
		cx: &mut task::Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		if !self.buf.is_empty() {
			buf.put(&mut pin!(self).buf);
			Poll::Ready(Ok(()))
		} else {
			Pin::new(&mut self.stream).poll_read(cx, buf)
		}
	}
}
