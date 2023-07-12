use anyhow::Context;
use bytes::{Buf, BytesMut};
use moq_generic_transport::{Connection, SendStream, SendStreamUnframed, RecvStream};
use moq_transport::{Decode, DecodeError, Encode, Object};
use std::{io::Cursor, marker::PhantomData};

use crate::SharedConnection;

// TODO support clients

// We could replace this generic soup by just <C: Connection> if we forced Connection's SendStream
// to provide SendStreamUnframes's send() method. Without that, we have to make Connection's
// SendStream type more specific and force it to implement SendStreamUnframes as well.
pub struct Objects<C: Connection> {
	send: SendObjects<C>,
	recv: RecvObjects<C>,
}

impl<S: SendStream + SendStreamUnframed, R: RecvStream + 'static, C: Connection<SendStream = S, RecvStream = R> + Send> Objects<C> {
	pub fn new(session: SharedConnection<C>) -> Self {
		let send = SendObjects::new(session.clone());
		let recv = RecvObjects::new(session);
		Self { send, recv }
	}

	pub fn split(self) -> (SendObjects<C>, RecvObjects<C>) {
		(self.send, self.recv)
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, R)> {
		self.recv.recv().await
	}

	pub async fn send(&mut self, header: Object) -> anyhow::Result<C::SendStream> {
		self.send.send(header).await
	}
}

pub struct SendObjects<C: Connection> {
	session: SharedConnection<C>,

	// A reusable buffer for encoding messages.
	buf: BytesMut,
	_marker: PhantomData<C>,
}

impl<S: SendStream + SendStreamUnframed, C: Connection<SendStream = S>> SendObjects<C> {
	pub fn new(session: SharedConnection<C>) -> Self {
		Self {
			session,
			buf: BytesMut::new(),
    		_marker: PhantomData,
		}
	}

	pub async fn send(&mut self, header: Object) -> anyhow::Result<C::SendStream> {
		self.buf.clear();
		header.encode(&mut self.buf).unwrap();

		// TODO support select! without making a new stream.
		let mut stream = moq_generic_transport::open_send_shared(self.session.clone())
			.await
			.context("failed to open uni stream")?;

		moq_generic_transport::send(&mut stream, &mut self.buf).await?;

		Ok(stream)
	}
}

impl<S: SendStream + SendStreamUnframed, C: Connection<SendStream = S>> Clone for SendObjects<C> {
	fn clone(&self) -> Self {
		Self {
			session: self.session.clone(),
			buf: BytesMut::new(),
    		_marker: PhantomData,
		}
	}
}

// Not clone, so we don't accidentally have two listners.
pub struct RecvObjects<C: Connection> {
	session: SharedConnection<C>,

	// A uni stream that's been accepted but not fully read from yet.
	stream: Option<Box<C::RecvStream>>,

	// Data that we've read but haven't formed a full message yet.
	buf: BytesMut,
}

impl<R: RecvStream + 'static, C: Connection<RecvStream = R>> RecvObjects<C> {
	pub fn new(session: SharedConnection<C>) -> Self {
		Self {
			session,
			stream: None,
			buf: BytesMut::new(),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, R)> {
		// Make sure any state is saved across await boundaries so this works with select!

		let stream = match self.stream.as_mut() {
			Some(stream) => stream,
			None => {
				let stream = moq_generic_transport::accept_recv_shared(self.session.clone())
				.await
				.context("failed to accept uni stream")?
				.context("no uni stream")?;

				self.stream.insert(Box::new(stream))
			}
		};

		loop {
			// Read the contents of the buffer
			let mut peek = Cursor::new(&self.buf);

			match Object::decode(&mut peek) {
				Ok(header) => {
					let stream = self.stream.take().unwrap();
					self.buf.advance(peek.position() as usize);
					
					return Ok((header, *stream));
				}
				Err(DecodeError::UnexpectedEnd) => {
					// The decode failed, so we need to append more data.
					moq_generic_transport::recv(stream.as_mut(), &mut self.buf).await?;
				}
				Err(e) => return Err(e.into()),
			}
		}
	}
}
