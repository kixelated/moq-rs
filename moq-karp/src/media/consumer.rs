use crate::catalog;

use crate::{media::Frame, util::FuturesExt};

use super::{Error, Timestamp};

use moq_transfork::{coding::*, Path, Session};

#[derive(Clone)]
pub struct BroadcastConsumer {
	catalog: catalog::Broadcast,
	session: Session,
	path: Path,
}

impl BroadcastConsumer {
	pub async fn load(mut session: Session, path: Path) -> Result<Self, Error> {
		let catalog = path.clone().push("catalog.json");
		let catalog = catalog::Broadcast::fetch(&mut session, catalog).await?;

		Ok(Self { session, catalog, path })
	}

	// This API could be improved
	pub async fn video(&self, name: &str) -> Result<TrackConsumer, Error> {
		let info = self.find_video(name)?;

		let track = moq_transfork::Track {
			path: self.path.clone().push(name),
			priority: info.track.priority,
			..Default::default()
		};
		let track = self.session.subscribe(track);

		Ok(TrackConsumer::new(track))
	}

	pub async fn audio(&self, name: &str) -> Result<TrackConsumer, Error> {
		let info = self.find_audio(name)?;

		let track = moq_transfork::Track {
			path: self.path.clone().push(name),
			priority: info.track.priority,
			..Default::default()
		};
		let track = self.session.subscribe(track);

		Ok(TrackConsumer::new(track))
	}

	fn find_audio(&self, name: &str) -> Result<&catalog::Audio, Error> {
		for audio in &self.catalog.audio {
			if audio.track.name == name {
				return Ok(audio);
			}
		}

		Err(Error::MissingTrack)
	}

	fn find_video(&self, name: &str) -> Result<&catalog::Video, Error> {
		for video in &self.catalog.video {
			if video.track.name == name {
				return Ok(video);
			}
		}

		Err(Error::MissingTrack)
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		&self.catalog
	}
}

pub struct TrackConsumer {
	track: moq_transfork::TrackConsumer,
	group: Option<moq_transfork::GroupConsumer>,
}

impl TrackConsumer {
	fn new(track: moq_transfork::TrackConsumer) -> Self {
		Self { track, group: None }
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
		let micros = u64::decode(&mut payload)?;
		let timestamp = Timestamp::from_micros(micros);

		let frame = Frame {
			keyframe,
			timestamp,
			payload,
		};

		Ok(frame)
	}
}
