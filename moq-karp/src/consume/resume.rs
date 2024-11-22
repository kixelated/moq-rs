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
	latest_id: Option<u64>,
}

impl Resumable {
	pub fn new(session: Session, path: Path) -> Self {
		let announced = session.announced_prefix(path);

		Self {
			session,
			announced,
			latest: Default::default(),
			latest_id: Default::default(),
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
		let suffix = path.strip_prefix(self.announced.prefix()).unwrap();
		if suffix.is_empty() {
			return Ok(None);
		}

		let id = suffix.first().ok_or(Error::InvalidSession)?;
		let id = u64::decode(&mut id.as_bytes())?;

		if let Some(latest_id) = &self.latest_id {
			if id <= *latest_id {
				// Skip old broadcasts
				return Ok(None);
			}
		}

		let base = self.announced.prefix().clone().push(suffix[0].clone());

		let closed = mpsc::channel(1);
		let broadcast = Broadcast::load_resumable(self.session.clone(), base, closed.0).await?;

		self.latest = Some((broadcast.clone(), closed.1));
		self.latest_id = Some(id);

		Ok(Some(broadcast))
	}

	fn unload(&mut self, path: Path) {
		self.latest.take_if(|(broadcast, _)| broadcast.path == path);
	}
}
