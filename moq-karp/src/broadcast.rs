use crate::{Audio, Catalog, Result, Track, TrackConsumer, TrackProducer, Video};

use moq_transfork::{Announced, AnnouncedConsumer, Path, Session};

use derive_more::Debug;

#[derive(Debug)]
#[debug("{:?}", path)]
pub struct BroadcastProducer {
	pub session: Session,
	pub path: Path,
	id: u128,

	catalog: Catalog,
	catalog_producer: moq_transfork::TrackProducer, // need to hold the track to keep it open
}

impl BroadcastProducer {
	pub fn new(mut session: Session, path: Path) -> Result<Self> {
		// Generate a "unique" ID for this broadcast session.
		// If we crash, then the viewers will automatically reconnect to the new ID.
		let id = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.unwrap()
			.as_millis();

		let full = path.clone().push(id);

		let track = moq_transfork::Track {
			path: full,
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		}
		.produce();

		session.publish(track.1)?;

		Ok(Self {
			session,
			path,
			id,
			catalog: Catalog::default(),
			catalog_producer: track.0,
		})
	}

	pub fn video(&mut self, info: Video) -> Result<TrackProducer> {
		let path = self.path.clone().push(self.id).push(&info.track.name);

		let (producer, consumer) = moq_transfork::Track {
			path,
			priority: info.track.priority,
			// TODO add these to the catalog and support higher latencies.
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		}
		.produce();

		self.session.publish(consumer)?;
		self.catalog.video.push(info);
		self.update()?;

		Ok(TrackProducer::new(producer))
	}

	pub fn audio(&mut self, info: Audio) -> Result<TrackProducer> {
		let path = self.path.clone().push(self.id).push(&info.track.name);

		let (producer, consumer) = moq_transfork::Track {
			path,
			priority: info.track.priority,
			// TODO add these to the catalog and support higher latencies.
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		}
		.produce();

		self.session.publish(consumer)?;
		self.catalog.audio.push(info);
		self.update()?;

		Ok(TrackProducer::new(producer))
	}

	pub fn catalog(&self) -> &Catalog {
		&self.catalog
	}

	fn update(&mut self) -> Result<()> {
		let frame = self.catalog.to_string()?;

		let mut group = self.catalog_producer.append_group();
		group.write_frame(frame);

		Ok(())
	}
}

// A broadcast consumer, supporting the ability to reload the catalog potentially on a crash.
#[derive(Debug)]
#[debug("{:?}", path)]
pub struct BroadcastConsumer {
	pub session: Session,
	pub path: Path,

	// Discovers new broadcasts as they are announced.
	announced: AnnouncedConsumer,
	announced_current: AnnouncedConsumer,

	// The ID of the current broadcast
	current: Option<String>,

	catalog_track: Option<moq_transfork::TrackConsumer>,
	catalog_group: Option<moq_transfork::GroupConsumer>,
}

impl BroadcastConsumer {
	pub fn new(session: Session, path: Path) -> Self {
		let announced = session.announced_prefix(path.clone());

		// Use a separate Consumer just to satisfy the borrow checker.
		let announced_current = announced.clone();

		/*
		let track = moq_transfork::Track {
			path: path.clone(),
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		};

		let catalog_track = session.subscribe(track);
		tracing::info!(?path, "fetching catalog");
		*/

		Self {
			session,
			path,
			announced,
			announced_current,
			current: None,
			catalog_track: None,
			catalog_group: None,
		}
	}

	/// Returns the latest catalog, or None if the broadcast has ended.
	// TODO Make a new interface instead of returning the catalog directly.
	// Otherwise, consumers won't realize that the underlying tracks are completely different.
	// ex. "video" on the old session !== "video" on the new session
	pub async fn catalog(&mut self) -> Result<Option<Catalog>> {
		loop {
			tokio::select! {
				biased;
				// Wait for new announcements.
				Some(announced) = self.announced.next() => {
					if announced.suffix().len() != 1 {
						// Ignore sub-tracks for a broadcast.
						continue
					}

					let id = announced.suffix().first().unwrap().clone();

					// Load or unload based on the announcement.
					match announced {
						Announced::Active(_) => self.load(id),
						Announced::Ended(_) => self.unload(id),
					}
				},
				// Wait until we're caught up on all announcements.
				true = self.announced_current.current(), if self.catalog_track.is_none() => {
					// Return a 404 if we couldn't find the broadcast.
					return Ok(None);
				},
				Some(group) = async { self.catalog_track.as_mut()?.next_group().await.transpose() } => {
					// Use the new group.
					self.catalog_group.replace(group?);
				},
				Some(frame) = async { self.catalog_group.as_mut()?.read_frame().await.transpose() } => {
					let catalog = Catalog::from_slice(&frame?)?;
					self.catalog_group.take(); // We don't support deltas yet
					return Ok(Some(catalog));
				},
				else => return Ok(None),
			}
		}
	}

	fn load(&mut self, id: String) {
		if let Some(current) = &self.current {
			// I'm extremely lazy and using string comparison.
			// This will be wrong when the number of milliseconds since 1970 adds a new digit...
			// But the odds of that happening are low.
			if id <= *current {
				tracing::warn!(?id, ?current, "ignoring old broadcast");
				return;
			}
		}

		let path = self.announced.prefix().clone().push(&id);
		tracing::info!(?path, "loading catalog");

		let track = moq_transfork::Track {
			path,
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		};

		self.catalog_track = Some(self.session.subscribe(track));
		self.current = Some(id);
	}

	fn unload(&mut self, id: String) {
		if self.current == Some(id) {
			self.current = None;
			self.catalog_track = None;
			self.catalog_group = None;
		}
	}

	/// Subscribes to a track
	pub fn track(&self, track: &Track) -> TrackConsumer {
		let track = moq_transfork::Track {
			path: self.path.clone().push(&track.name),
			priority: track.priority,

			// TODO add these to the catalog and support higher latencies.
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		};
		let track = self.session.subscribe(track);
		TrackConsumer::new(track)
	}
}
