use crate::Error;

use super::Broadcast;

use moq_transfork::{coding::*, Announced, AnnouncedConsumer, Path, Session};
use tokio::sync::mpsc;

/// Provides resumable access to broadcasts.
/// Each broadcast is identified by an increasing ID, allowing the consumer to discover crashes and use the higher ID.
pub struct Resumable {
	session: Session,

	announced: AnnouncedConsumer,
	latest: Option<(Broadcast, mpsc::Receiver<()>)>,
}

impl Resumable {
	pub fn new(session: Session, path: Path) -> Self {
		let announced = session.announced_prefix(path);

		Self {
			session,
			announced,
			latest: Default::default(),
		}
	}

	// Returns the next discovered broadcast.
	// The consumer is responsible for (gracefully) terminating the previous broadcast.
	pub async fn broadcast(&mut self) -> Option<Broadcast> {
		while let Some(broadcast) = self.announced.next().await {
			match broadcast {
				Announced::Active(path) => match self.try_load(path).await {
					Ok(consumer) => return consumer,
					Err(err) => tracing::warn!(?err, "failed to load broadcast"),
				},
				Announced::Ended(path) => self.unload(path),
			}
		}

		None
	}

	async fn try_load(&mut self, path: Path) -> Result<Option<Broadcast>, Error> {
		let id = self.id(&path).ok_or(Error::InvalidSession)?;

		if let Some((latest, _)) = &self.latest {
			let latest = self.id(&latest.path).ok_or(Error::InvalidSession)?;
			if id <= latest {
				// Skip old broadcasts
				return Ok(None);
			}
		}

		let closed = mpsc::channel(1);
		let broadcast = Broadcast::load_resumable(self.session.clone(), path, closed.0).await?;

		self.latest = Some((broadcast.clone(), closed.1));

		Ok(Some(broadcast))
	}

	fn unload(&mut self, path: Path) {
		self.latest.take_if(|(broadcast, _)| broadcast.path == path);
	}

	// Returns the numberic session ID of the broadcast.
	fn id(&self, path: &Path) -> Option<u64> {
		let id = path.get(self.announced.prefix().len())?;
		u64::decode(&mut id.as_bytes()).ok()
	}
}
