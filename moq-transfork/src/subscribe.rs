use std::ops;

use crate::coding::Stream;
use crate::message;
use crate::TrackReader;

use super::SessionError;

pub struct Subscribe {
	msg: message::Subscribe,
	track: TrackReader,
}

impl Subscribe {
	pub(super) fn new(msg: message::Subscribe, track: TrackReader) -> Self {
		Self { msg, track }
	}

	pub async fn run(&mut self, session: web_transport::Session) -> Result<(), SessionError> {
		tokio::select! {
			res = self.track.closed() => res?,
			res = self.run_inner(session) => res?,
		};

		Ok(())
	}

	pub async fn run_inner(&self, mut session: web_transport::Session) -> Result<(), SessionError> {
		let mut control = Stream::open(&mut session, message::Control::Subscribe).await?;
		control.writer.encode(&self.msg).await?;

		let info: message::Info = control.reader.decode().await?;

		while let Some(dropped) = control.reader.decode_maybe::<message::GroupDrop>().await? {
			// TODO expose to application
		}

		// TODO allow the application to update the subscription

		Ok(())
	}
}

impl ops::Deref for Subscribe {
	type Target = message::Subscribe;

	fn deref(&self) -> &Self::Target {
		&self.msg
	}
}
