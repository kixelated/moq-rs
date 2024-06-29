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

	#[tracing::instrument("subscribe", skip(self), fields(stream = &self.stream.id()))]
	pub async fn start(&mut self) -> Result<(), SessionError> {
		let res = self.start_inner().await;
		if let Err(err) = &res {
			tracing::warn!(?err);
			self.stream.close(err.code());
		}

		res
	}

	async fn start_inner(&mut self) -> Result<(), SessionError> {
		let request = message::Subscribe {
			id: self.id,
			broadcast: self.track.broadcast.to_string(),

			track: self.track.name.clone(),
			priority: self.track.priority,

			group_order: self.track.group_order,
			group_expires: self.track.group_expires,

			// TODO
			group_min: None,
			group_max: None,
		};

		tracing::info!(?request);

		self.stream.writer.encode(&request).await?;

		// TODO use the response to update the track
		let response: message::Info = self.stream.reader.decode().await?;
		tracing::info!(?response);

		Ok(())
	}

	// TODO allow the application to update the subscription
	#[tracing::instrument("subscribe", skip(self), fields(stream = &self.stream.id()))]
	pub async fn run(&mut self) {
		let res = self.run_inner().await;
		if let Err(err) = &res {
			tracing::warn!(?err);
			self.stream.close(err.code());
		}
	}

	async fn run_inner(&mut self) -> Result<(), SessionError> {
		loop {
			tokio::select! {
				res = self.stream.reader.decode_maybe::<message::GroupDrop>() => {
					// TODO expose updates to application
					match res? {
						Some(drop) => tracing::info!(?drop),
						None => return Ok(()),
					}
				},
				res = self.track.closed() => res?,
			};
		}
	}
}
