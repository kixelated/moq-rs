use std::fmt;

use crate::{
	coding::{self, Stream},
	message, model,
};

use super::SessionError;

pub struct Subscribe {
	pub id: u64,
	track: model::TrackReader,
	stream: coding::Stream,
}

impl Subscribe {
	pub async fn open(
		session: &mut web_transport::Session,
		id: u64,
		track: model::TrackReader,
	) -> Result<Self, SessionError> {
		let stream = Stream::open(session, message::Control::Subscribe).await?;
		let mut this = Self { id, track, stream };

		if let Err(err) = this.open_inner().await {
			this.stream.writer.reset(err.code());
			return Err(err);
		}

		Ok(this)
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(broadcast=self.track.broadcast, track=self.track.name, stream = self.stream.id))]
	async fn open_inner(&mut self) -> Result<(), SessionError> {
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

	pub async fn run(mut self) {
		if let Err(err) = self.run_inner().await {
			self.stream.writer.reset(err.code());
		}
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(broadcast=self.track.broadcast, track=self.track.name, stream = self.stream.id))]
	pub async fn run_inner(&mut self) -> Result<(), SessionError> {
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
