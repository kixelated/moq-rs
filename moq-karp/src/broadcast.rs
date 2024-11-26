use std::collections::HashSet;

use crate::{Audio, Catalog, Error, Result, Track, TrackConsumer, TrackProducer, Video};

use moq_transfork::{Announced, AnnouncedConsumer, Path, Session};

use derive_more::Debug;

#[derive(Debug)]
#[debug("{:?}", path)]
pub struct BroadcastProducer {
	session: Session,
	path: Path,

	catalog: Catalog,
	catalog_producer: moq_transfork::TrackProducer, // need to hold the track to keep it open
}

impl BroadcastProducer {
	pub fn new(mut session: Session, path: Path) -> Result<Self> {
		let track = moq_transfork::Track {
			path: path.clone(),
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		}
		.produce();

		session.publish(track.1)?;

		Ok(Self {
			session,
			path,
			catalog: Catalog::default(),
			catalog_producer: track.0,
		})
	}

	pub fn video(&mut self, info: Video) -> Result<TrackProducer> {
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.track.name),
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
		let (producer, consumer) = moq_transfork::Track {
			path: self.path.clone().push(&info.track.name),
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

/// Provides resumable access to broadcasts.
/// Each broadcast is identified by an increasing ID, allowing the consumer to discover crashes and use the higher ID.
#[derive(Debug)]
#[debug("{:?}", announced.prefix())]
pub struct BroadcastAnnounced {
	session: Session,
	announced: AnnouncedConsumer,
	active: HashSet<String>,
}

impl BroadcastAnnounced {
	pub fn new(session: Session, path: Path) -> Self {
		let announced = session.announced_prefix(path);

		Self {
			session,
			announced,
			active: HashSet::new(),
		}
	}

	// Returns the next unique broadcast from this user.
	pub async fn broadcast(&mut self) -> Option<BroadcastConsumer> {
		while let Some(suffix) = self.announced.next().await {
			match suffix {
				Announced::Active(suffix) => match self.try_load(suffix).await {
					Ok(consumer) => return consumer,
					Err(err) => tracing::warn!(?err, "failed to load broadcast"),
				},
				Announced::Ended(suffix) => self.unload(suffix),
			}
		}

		None
	}

	async fn try_load(&mut self, suffix: Path) -> Result<Option<BroadcastConsumer>> {
		let name = suffix.first().ok_or(Error::InvalidSession)?;
		tracing::info!(?name, "loading broadcast");

		if self.active.contains(name.as_str()) {
			// Skip duplicates
			return Ok(None);
		}

		let path = self.announced.prefix().clone().push(name);
		tracing::info!(prefix = ?self.announced.prefix(), ?path, "loading broadcast");
		let broadcast = BroadcastConsumer::new(self.session.clone(), path);

		self.active.insert(name.to_string());

		Ok(Some(broadcast))
	}

	fn unload(&mut self, suffix: Path) {
		let name = suffix.first().expect("invalid path");
		self.active.remove(name.as_str());
	}
}

#[derive(Debug)]
#[debug("{:?}", path)]
pub struct BroadcastConsumer {
	session: Session,
	path: Path,

	catalog_track: moq_transfork::TrackConsumer,
	catalog_group: Option<moq_transfork::GroupConsumer>,
}

impl BroadcastConsumer {
	pub fn new(session: Session, path: Path) -> Self {
		let track = moq_transfork::Track {
			path: path.clone(),
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		};

		let catalog_track = session.subscribe(track);
		tracing::info!(?path, "fetching catalog");

		Self {
			session,
			path,
			catalog_track,
			catalog_group: None,
		}
	}

	/// Returns the latest catalog
	pub async fn catalog(&mut self) -> Result<Option<Catalog>> {
		loop {
			tokio::select! {
				biased;
				Some(frame) = async { self.catalog_group.as_mut()?.read_frame().await.transpose() } => {
					let catalog = Catalog::from_slice(&frame?)?;
					self.catalog_group.take(); // We don't support deltas yet
					return Ok(Some(catalog));
				},
				Some(group) = async { self.catalog_track.next_group().await.transpose() } => {
					println!("{:?}", group);
					self.catalog_group.replace(group?);
				},
				else => return Ok(None),
			}
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
