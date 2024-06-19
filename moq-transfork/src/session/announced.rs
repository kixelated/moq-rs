use crate::BroadcastWriter;
use crate::{coding::Stream, message};

use super::SessionError;

pub struct Announced {
	broadcast: BroadcastWriter,
}

impl Announced {
	pub fn new(broadcast: BroadcastWriter) -> Self {
		Self { broadcast }
	}

	pub async fn run(self, stream: &mut Stream) -> Result<(), SessionError> {
		// Wait until either the reader or the session is closed.
		tokio::select! {
			res = stream.reader.closed() => res?,
			res = self.broadcast.closed() => res?,
		};

		// Send the OK message.
		let msg = message::AnnounceOk {};
		stream.writer.encode(&msg).await?;

		// Wait until the reader is closed.
		tokio::select! {
			res = stream.reader.closed() => res?,
			res = self.broadcast.closed() => res?,
		}

		// TODO reset with the correct error code

		Ok(())
	}
}
