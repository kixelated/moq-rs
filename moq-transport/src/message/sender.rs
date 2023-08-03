use crate::coding::Encode;
use crate::message::Message;

use bytes::BytesMut;

use webtransport_generic::AsyncSendStream;

pub struct Sender<S>
where
	S: AsyncSendStream, // TODO take SendStream instead
{
	stream: S,
	buf: BytesMut, // reuse a buffer to encode messages.
}

impl<S> Sender<S>
where
	S: AsyncSendStream,
{
	pub fn new(stream: S) -> Self {
		Self {
			buf: BytesMut::new(),
			stream,
		}
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();

		self.buf.clear();
		msg.encode(&mut self.buf)?;

		self.stream.send(&mut self.buf).await?;

		Ok(())
	}

	/*
	// Helper that lets multiple threads send control messages.
	pub fn share(self) -> ControlShared<S> {
		ControlShared {
			stream: Arc::new(Mutex::new(self)),
		}
	}
	*/
}

/*
// Helper that allows multiple threads to send control messages.
// There's no equivalent for receiving since only one thread should be receiving at a time.
#[derive(Clone)]
pub struct SendControlShared<S>
where
	S: AsyncSendStream,
{
	stream: Arc<Mutex<SendControl<S>>>,
}

impl<S> SendControlShared<S>
where
	S: AsyncSendStream,
{
	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let mut stream = self.stream.lock().await;
		stream.send(msg).await
	}
}
*/
