use crate::{Audio, Catalog, Result, Track, TrackConsumer, TrackProducer, Video};

use moq_transfork::{Path, Session};

pub struct Broadcast {
	pub session: Session,
	pub path: Path,
}

impl Broadcast {
	pub fn new(session: Session, path: Path) -> Self {
		Self { session, path }
	}

	pub fn produce(self) -> Result<BroadcastProducer> {
		BroadcastProducer::new(self)
	}

	pub fn consume(self) -> BroadcastConsumer {
		BroadcastConsumer::new(self)
	}
}

pub struct BroadcastProducer {
	broadcast: Broadcast,
	catalog: Catalog,
	catalog_producer: moq_transfork::TrackProducer, // need to hold the track to keep it open
}

impl BroadcastProducer {
	pub fn new(mut broadcast: Broadcast) -> Result<Self> {
		let track = moq_transfork::Track {
			path: broadcast.path.clone().push("catalog.json"),
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		}
		.produce();

		broadcast.session.publish(track.1)?;

		Ok(Self {
			broadcast,
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

		self.broadcast.session.publish(consumer)?;
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

		self.broadcast.session.publish(consumer)?;
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

impl std::ops::Deref for BroadcastProducer {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.broadcast
	}
}

pub struct BroadcastConsumer {
	broadcast: Broadcast,

	catalog_track: moq_transfork::TrackConsumer,
	catalog_group: Option<moq_transfork::GroupConsumer>,
}

impl BroadcastConsumer {
	pub fn new(broadcast: Broadcast) -> Self {
		let path = broadcast.path.clone().push("catalog.json");

		let track = moq_transfork::Track {
			path,
			priority: -1,
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		};

		let catalog_track = broadcast.session.subscribe(track);

		Self {
			broadcast,
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
					self.catalog_group.replace(group?);
				},
				else => return Ok(None),
			}
		}
	}

	/// Subscribes to a track
	pub fn track(&self, track: &Track) -> TrackConsumer {
		let track = moq_transfork::Track {
			path: self.broadcast.path.clone().push(&track.name),
			priority: track.priority,

			// TODO add these to the catalog and support higher latencies.
			group_order: moq_transfork::GroupOrder::Desc,
			group_expires: std::time::Duration::ZERO,
		};
		let track = self.session.subscribe(track);
		TrackConsumer::new(track)
	}
}

impl std::ops::Deref for BroadcastConsumer {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.broadcast
	}
}
