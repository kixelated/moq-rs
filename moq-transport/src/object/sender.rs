use anyhow::Context;

use crate::Object;

use webtransport_generic::{SendStream, Session};

// Allow this to be cloned so we can have multiple senders.
#[derive(Clone)]
pub struct Sender<S: Session> {
	// The session.
	session: S,
}

impl<S: Session> Sender<S> {
	pub fn new(session: S) -> Self {
		Self { session }
	}

	pub async fn open(&mut self, object: Object) -> anyhow::Result<S::SendStream> {
		let mut stream = self.session.open_uni().await.context("failed to open uni stream")?;

		stream.set_priority(object.send_order);
		object.encode(&mut stream).await.context("failed to write header")?;

		// log::info!("created stream: {:?}", header);

		Ok(stream)
	}
}
