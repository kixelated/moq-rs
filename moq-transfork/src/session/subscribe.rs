use crate::{coding, message, model};

use super::SessionError;

pub struct Subscribe {
	pub id: u64,
	stream: coding::Stream,
	track: model::TrackReader,
}

impl Subscribe {
	pub(super) fn new(id: u64, stream: coding::Stream, track: model::TrackReader) -> Self {
		Self { id, stream, track }
	}

	// TODO allow the application to update the subscription
	pub async fn run(&mut self) {
		let res = self.run_inner().await;
		if let Err(err) = &res {
			self.stream.writer.reset(err.code());
		}
	}

	async fn run_inner(&mut self) -> Result<(), SessionError> {
		loop {
			tokio::select! {
				res = self.stream.reader.decode_maybe::<message::GroupDrop>() => {
					// TODO expose updates to application
					if res?.is_none() {
						return Ok(())
					}
				},
				res = self.track.closed() => res?,
			};
		}
	}
}
