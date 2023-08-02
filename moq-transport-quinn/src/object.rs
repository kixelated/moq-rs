use std::{collections::BinaryHeap, io::Cursor, sync::Arc};

use anyhow::Context;
use bytes::{Buf, BytesMut};
use moq_transport::{Decode, DecodeError, Encode, Object};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use webtransport_quinn::Session;

use crate::{RecvStream, SendStream, SendStreamOrder};

// Allow this to be cloned so we can have multiple senders.
#[derive(Clone)]
pub struct SendObjects {
	// This is a tokio mutex since we need to lock across await boundaries.
	inner: Arc<Mutex<SendObjectsInner>>,
}

impl SendObjects {
	pub fn new(session: Session) -> Self {
		let inner = SendObjectsInner::new(session);
		Self {
			inner: Arc::new(Mutex::new(inner)),
		}
	}

	pub async fn open(&mut self, header: Object) -> anyhow::Result<SendStream> {
		let mut inner = self.inner.lock().await;
		inner.open(header).await
	}
}

struct SendObjectsInner {
	session: Session,

	// Quinn supports a i32 for priority, but the wire format is a u64.
	// Our work around is to keep a list of streams in priority order and use the index as the priority.
	// This involves more work, so TODO either increase the Quinn size or reduce the wire size.
	ordered: BinaryHeap<SendStreamOrder>,
	ordered_swap: BinaryHeap<SendStreamOrder>, // reuse memory to avoid allocations

	// A reusable buffer for encoding headers.
	// TODO figure out how to use BufMut on the stack and remove this.
	buf: BytesMut,
}

impl SendObjectsInner {
	fn new(session: Session) -> Self {
		Self {
			session,
			ordered: BinaryHeap::new(),
			ordered_swap: BinaryHeap::new(),
			buf: BytesMut::new(),
		}
	}

	pub async fn open(&mut self, header: Object) -> anyhow::Result<SendStream> {
		let stream = self.session.open_uni().await.context("failed to open uni stream")?;
		let (mut stream, priority) = SendStream::with_order(stream, header.send_order.into_inner());

		// Add the priority to our existing list.
		self.ordered.push(priority);

		// Loop through the list and update the priorities of any still active streams.
		let mut index = 0;
		while let Some(stream) = self.ordered.pop() {
			if stream.update(index).is_ok() {
				// Add the stream to the new list so it'll be in sorted order.
				self.ordered_swap.push(stream);
				index += 1;
			}
		}

		// Swap the lists so we can reuse the memory.
		std::mem::swap(&mut self.ordered, &mut self.ordered_swap);

		// Encode and write the stream header.
		// TODO do this in SendStream so we don't hold the lock.
		// Otherwise,
		self.buf.clear();
		header.encode(&mut self.buf).unwrap();
		stream.write_all(&self.buf).await.context("failed to write header")?;

		// log::info!("created stream: {:?}", header);

		Ok(stream)
	}
}

// Not clone, so we don't accidentally have two listners.
pub struct RecvObjects {
	session: Session,

	// Streams that we've accepted but haven't read the header from yet.
	streams: JoinSet<anyhow::Result<(Object, RecvStream)>>,
}

impl RecvObjects {
	pub fn new(session: Session) -> Self {
		Self {
			session,
			streams: JoinSet::new(),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, RecvStream)> {
		loop {
			tokio::select! {
				res = self.session.accept_uni() => {
					let stream = res.context("failed to accept stream")?;
					self.streams.spawn(async move { Self::read(stream).await });
				},
				res = self.streams.join_next(), if !self.streams.is_empty() => {
					return res.unwrap().context("failed to run join set")?;
				}
			}
		}
	}

	async fn read(mut stream: webtransport_quinn::RecvStream) -> anyhow::Result<(Object, RecvStream)> {
		let mut buf = BytesMut::new();

		loop {
			// Read more data into the buffer.
			stream.read_buf(&mut buf).await?;

			// Use a cursor to read the buffer and remember how much we read.
			let mut read = Cursor::new(&mut buf);

			let header = match Object::decode(&mut read) {
				Ok(header) => header,
				Err(DecodeError::UnexpectedEnd) => continue,
				Err(err) => return Err(err.into()),
			};

			// We parsed a full header, advance the buffer.
			let size = read.position() as usize;
			buf.advance(size);
			let buf = buf.freeze();

			// log::info!("received stream: {:?}", header);

			let stream = RecvStream::new(buf, stream);

			return Ok((header, stream));
		}
	}
}
