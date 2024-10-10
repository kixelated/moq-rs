use crate::catalog;

use crate::{
	media::{Frame, Timestamp},
	util::FuturesExt,
};

use super::Error;

use moq_transfork::coding::*;

#[derive(Clone)]
pub struct BroadcastConsumer {
	catalog: catalog::Broadcast,
	inner: moq_transfork::BroadcastConsumer,
}

impl BroadcastConsumer {
	pub async fn load(broadcast: moq_transfork::BroadcastConsumer) -> Result<Self, Error> {
		let catalog = catalog::Broadcast::fetch(broadcast.clone()).await?;

		Ok(Self {
			inner: broadcast,
			catalog,
		})
	}

	// This API could be improved
	pub async fn subscribe<T: Into<moq_transfork::Track>>(&self, track: T) -> Result<TrackConsumer, Error> {
		let track = track.into();

		// Make sure the track exists in the catalog.
		// We need it for the timescale.
		let mut timescale = None;

		for audio in &self.catalog.audio {
			if audio.track == track {
				timescale = Some(audio.timescale);
			}
		}

		for video in &self.catalog.video {
			if video.track == track {
				timescale = Some(video.timescale);
			}
		}

		let timescale = timescale.ok_or(Error::MissingTrack)?;

		let track = self.inner.get_track(track).await?;
		Ok(TrackConsumer::new(track, timescale))
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		&self.catalog
	}
}

pub struct TrackConsumer {
	track: moq_transfork::TrackConsumer,
	timescale: u32,

	group: Option<moq_transfork::GroupConsumer>,
}

impl TrackConsumer {
	fn new(track: moq_transfork::TrackConsumer, timescale: u32) -> Self {
		Self {
			track,
			timescale,
			group: None,
		}
	}

	pub async fn read(&mut self) -> Result<Option<Frame>, Error> {
		let mut keyframe = false;

		if self.group.is_none() {
			self.group = self.track.next_group().await?;
			keyframe = true;

			if self.group.is_none() {
				return Ok(None);
			}
		}

		loop {
			tokio::select! {
				biased;
				Some(res) = self.group.as_mut().unwrap().read_frame().transpose() => {
					let raw = res?;
					let frame =  self.decode_frame(raw, keyframe)?;
					return Ok(Some(frame));
				},
				Some(res) = self.track.next_group().transpose() => {
					let group = res?;

					if group.sequence < self.group.as_ref().unwrap().sequence {
						// Ignore old groups
						continue;
					}

					// TODO use a configurable latency before moving to the next group.
					self.group = Some(group);
					keyframe = true;
				},
				else => return Ok(None),
			}
		}
	}

	fn decode_frame(&self, mut payload: Bytes, keyframe: bool) -> Result<Frame, Error> {
		let base = u64::decode(&mut payload)?;
		let timestamp = Timestamp::from_scale(base, self.timescale as _);

		let frame = Frame {
			keyframe,
			timestamp,
			payload,
		};

		Ok(frame)
	}
}
