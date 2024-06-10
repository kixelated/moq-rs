use std::ops;

use crate::{coding::Stream, message, BroadcastReader};

use super::SessionError;

pub struct Announce {
	msg: message::Announce,
	broadcast: BroadcastReader,
}

impl Announce {
	pub fn new(msg: message::Announce, broadcast: BroadcastReader) -> Self {
		Self { msg, broadcast }
	}

	pub async fn run(&mut self, session: web_transport::Session) -> Result<(), SessionError> {
		tokio::select! {
			res = self.run_inner(session) => res?,
			res = self.broadcast.closed() => res?,
		};

		Ok(())
	}

	pub async fn run_inner(&self, mut session: web_transport::Session) -> Result<(), SessionError> {
		let mut stream = Stream::open(&mut session, message::Control::Announce).await?;

		// TODO handle errors
		stream.writer.encode(&self.msg).await?;
		let _ = stream.reader.decode::<message::AnnounceOk>().await;
		stream.reader.closed().await?;

		Ok(())
	}
}

impl ops::Deref for Announce {
	type Target = message::Announce;

	fn deref(&self) -> &Self::Target {
		&self.msg
	}
}
