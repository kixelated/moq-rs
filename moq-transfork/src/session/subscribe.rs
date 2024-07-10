use crate::{message, model, Broadcast, TrackReader};

use super::{OrClose, SessionError, Stream};

pub struct Subscribe {
	pub id: u64,
	broadcast: Broadcast,
	track: model::TrackReader,
	stream: Stream,
}

impl Subscribe {
	pub fn new(stream: Stream, id: u64, broadcast: Broadcast, track: TrackReader) -> Self {
		Self {
			id,
			track,
			broadcast,
			stream,
		}
	}

	pub async fn start(&mut self) -> Result<(), SessionError> {
		self.start_inner().await.or_close(&mut self.stream)
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(broadcast=self.broadcast.name, track=self.track.name, id = self.id))]
	async fn start_inner(&mut self) -> Result<(), SessionError> {
		let request = message::Subscribe {
			id: self.id,
			broadcast: self.broadcast.name.clone(),

			track: self.track.name.clone(),
			priority: self.track.priority,

			group_order: self.track.group_order,
			group_expires: self.track.group_expires,

			// TODO
			group_min: None,
			group_max: None,
		};

		self.stream.writer.encode(&request).await?;

		// TODO use the response to update the track
		let _response: message::Info = self.stream.reader.decode().await?;

		tracing::info!("ok");

		Ok(())
	}

	pub async fn run(mut self) -> Result<(), SessionError> {
		self.run_inner().await.or_close(&mut self.stream)
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(broadcast = self.broadcast.name, track=self.track.name, id = self.id))]
	pub async fn run_inner(&mut self) -> Result<(), SessionError> {
		loop {
			tokio::select! {
				res = self.stream.reader.decode_maybe::<message::GroupDrop>() => {
					// TODO expose updates to application
					// TODO use to detect gaps
					if res?.is_none() {
						return Ok(());
					}
				},
				res = self.track.closed() => res?,
			};
		}
	}
}
