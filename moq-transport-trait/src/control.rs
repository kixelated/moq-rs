use moq_generic_transport::{SendStream, RecvStream, BidiStream, SendStreamUnframed};
use moq_transport::{Decode, DecodeError, Encode, Message};

use bytes::{Buf, BytesMut};

use std::io::Cursor;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::Mutex;


pub struct Control<S: SendStream + SendStreamUnframed, B: BidiStream<SendStream = S>> {
	sender: ControlSend<B::SendStream>,
	recver: ControlRecv<B::RecvStream>,
}

impl<S: SendStream + SendStreamUnframed, B: BidiStream<SendStream = S>> Control<S, B> {
	pub(crate) fn new(stream: Box<B>) -> Self {
		let (sender, recver) = stream.split();
		let sender = ControlSend::new(Box::new(sender));
		let recver = ControlRecv::new(Box::new(recver));

		Self { sender, recver }
	}

	pub fn split(self) -> (ControlSend<B::SendStream>, ControlRecv<B::RecvStream>) {
		(self.sender, self.recver)
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		self.sender.send(msg).await
	}

	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		self.recver.recv().await
	}
}

pub struct ControlSend<S> {
	stream: Box<S>,
	buf: BytesMut, // reuse a buffer to encode messages.
}

impl<S: SendStream + SendStreamUnframed> ControlSend<S> {
	pub fn new(inner: Box<S>) -> Self {
		Self {
			buf: BytesMut::new(),
			stream: inner,
		}
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		log::info!("sending message: {:?}", msg);

		self.buf.clear();
		msg.encode(&mut self.buf)?;

		// TODO make this work with select!
		moq_generic_transport::send(self.stream.as_mut(), &mut self.buf).await?;

		Ok(())
	}

	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> ControlShared<S> {
		ControlShared {
			stream: Arc::new(Mutex::new(self)),
    		_marker: PhantomData,
		}
	}
}

// Helper that allows multiple threads to send control messages.
// There's no equivalent for receiving since only one thread should be receiving at a time.
#[derive(Clone)]
pub struct ControlShared<S: SendStream + SendStreamUnframed> {
	stream: Arc<Mutex<ControlSend<S>>>,
	_marker: PhantomData<S>
}

impl<S: SendStream + SendStreamUnframed> ControlShared<S> {
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let mut stream = self.stream.lock().await;
		stream.send(msg).await
	}
}

pub struct ControlRecv<R: RecvStream> {
	stream: Box<R>,
	buf: BytesMut, // data we've read but haven't fully decoded yet
}

impl<R: RecvStream> ControlRecv<R> {
	pub fn new(inner: Box<R>) -> Self {
		Self {
			buf: BytesMut::new(),
			stream: inner,
		}
	}

	// Read the next full message from the stream.
	pub async fn recv(&mut self) -> anyhow::Result<Message> {
		loop {
			// Read the contents of the buffer
			let mut peek = Cursor::new(&self.buf);

			match Message::decode(&mut peek) {
				Ok(msg) => {
					// We've successfully decoded a message, so we can advance the buffer.
					self.buf.advance(peek.position() as usize);

					log::info!("received message: {:?}", msg);
					return Ok(msg);
				}
				Err(DecodeError::UnexpectedEnd) => {
					// The decode failed, so we need to append more data.
					moq_generic_transport::recv(self.stream.as_mut(), &mut self.buf).await?;
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
