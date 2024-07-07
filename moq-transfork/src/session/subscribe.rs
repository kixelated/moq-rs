use crate::{message, model};

use super::{OrClose, SessionError, Stream};

pub struct Subscribe {
	pub id: u64,
	track: model::TrackReader,
	stream: Stream,
}

impl Subscribe {
	pub async fn open(
		session: &mut web_transport::Session,
		id: u64,
		track: model::TrackReader,
	) -> Result<Self, SessionError> {
		let stream = Stream::open(session, message::Stream::Subscribe).await?;
		let mut this = Self { id, track, stream };
		this.open_inner().await.or_close(&mut this.stream)?;
		Ok(this)
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(id=self.id, broadcast=self.track.broadcast, track=self.track.name))]
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

		self.stream.writer.encode(&request).await?;

		// TODO use the response to update the track
		let _response: message::Info = self.stream.reader.decode().await?;

		tracing::info!("ok");

		Ok(())
	}

	pub async fn run(mut self) {
		let _ = self.run_inner().await.or_close(&mut self.stream);
	}

	#[tracing::instrument("subscribe", skip_all, err, fields(id = self.id, broadcast=self.track.broadcast, track=self.track.name))]
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
