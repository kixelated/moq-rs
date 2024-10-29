use crate::{catalog, Error};

use moq_transfork::{Path, Session};
use tokio::sync::mpsc;

use super::Track;

#[derive(Clone)]
pub struct Broadcast {
	pub session: Session,
	pub path: Path,

	catalog: catalog::Broadcast,

	// Closed when there's a new broadcast to consume.
	resume: Option<mpsc::Sender<()>>,
}

impl Broadcast {
	/// Load a broadcast at the provided path.
	pub async fn load(mut session: Session, path: Path) -> Result<Broadcast, Error> {
		let catalog = path.clone().push("catalog.json");
		let catalog = catalog::Broadcast::fetch(&mut session, catalog).await?;

		Ok(Self {
			session,
			catalog,
			path,
			resume: None,
		})
	}

	pub(super) async fn load_resumable(
		session: Session,
		path: Path,
		closed: mpsc::Sender<()>,
	) -> Result<Broadcast, Error> {
		let mut this = Self::load(session.clone(), path.clone()).await?;
		this.resume = Some(closed);
		Ok(this)
	}

	// This API could be improved
	pub fn video(&self, name: &str) -> Result<Track, Error> {
		let info = self.find_video(name)?;

		let track = moq_transfork::Track {
			path: self.path.clone().push(name),
			priority: info.track.priority,
			..Default::default()
		};
		let track = self.session.subscribe(track);

		Ok(Track::new(track))
	}

	pub fn audio(&self, name: &str) -> Result<Track, Error> {
		let info = self.find_audio(name)?;

		let track = moq_transfork::Track {
			path: self.path.clone().push(name),
			priority: info.track.priority,
			..Default::default()
		};
		let track = self.session.subscribe(track);

		Ok(Track::new(track))
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

	/// Returns Ok() when the broadcast is closed because a newer one was discovered.
	pub async fn closed(&self) -> Result<(), Error> {
		if let Some(resume) = &self.resume {
			tokio::select! {
				res = self.session.closed() => Err(res.into()),
				_ = resume.closed() => Ok(()),
			}
		} else {
			Err(self.session.closed().await.into())
		}
	}
}
