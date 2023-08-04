use crate::message::Message;

use webtransport_generic::SendStream;

pub struct Sender<S: SendStream> {
	stream: S,
}

impl<S: SendStream> Sender<S> {
	pub fn new(stream: S) -> Self {
		Self { stream }
	}

	pub async fn send<T: Into<Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		let msg = msg.into();
		msg.encode(&mut self.stream).await?;
		Ok(())
	}
}
