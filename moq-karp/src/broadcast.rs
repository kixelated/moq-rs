use crate::{Audio, Catalog, Error, Result, Track, TrackConsumer, TrackProducer, Video};

use moq_async::{spawn, Lock};
use moq_transfork::{Announced, AnnouncedConsumer, AnnouncedMatch, Session};

use derive_more::Debug;

#[derive(Debug, Clone)]
#[debug("{:?}", path)]
pub struct BroadcastProducer {
	pub session: Session,
	pub path: String,
	id: u64,
	catalog: Lock<CatalogProducer>,
}

impl BroadcastProducer {
	pub fn new(mut session: Session, path: String) -> Result<Self> {
		// Generate a "unique" ID for this broadcast session.
		// If we crash, then the viewers will automatically reconnect to the new ID.
		let id = web_time::SystemTime::now()
			.duration_since(web_time::SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64;

		let full = format!("{}/{}", path, id);

		let catalog = moq_transfork::Track {
			path: full,
			priority: -1,
			order: moq_transfork::GroupOrder::Desc,
		}
		.produce();

		// Publish the catalog track, even if it's empty.
		session.publish(catalog.1)?;

		let catalog = Lock::new(CatalogProducer::new(catalog.0)?);

		Ok(Self {
			session,
			path,
			id,
			catalog,
		})
	}

	/// Return the latest catalog.
	pub fn catalog(&self) -> Catalog {
		self.catalog.lock().current.clone()
	}

	pub fn publish_video(&mut self, info: Video) -> Result<TrackProducer> {
		let path = format!("{}/{}/{}", self.path, self.id, &info.track.name);

		let (producer, consumer) = moq_transfork::Track {
			path,
			priority: info.track.priority,
			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		}
		.produce();

		self.session.publish(consumer)?;

		let mut catalog = self.catalog.lock();
		catalog.current.video.push(info.clone());
		catalog.publish()?;

		let producer = TrackProducer::new(producer);
		let consumer = producer.subscribe();

		// Start a task that will remove the catalog on drop.
		let catalog = self.catalog.clone();
		spawn(async move {
			consumer.closed().await.ok();

			let mut catalog = catalog.lock();
			catalog.current.video.retain(|v| v.track != info.track);
			catalog.publish().unwrap();
		});

		Ok(producer)
	}

	pub fn publish_audio(&mut self, info: Audio) -> Result<TrackProducer> {
		let path = format!("{}/{}/{}", self.path, self.id, &info.track.name);

		let (producer, consumer) = moq_transfork::Track {
			path,
			priority: info.track.priority,
			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		}
		.produce();

		self.session.publish(consumer)?;

		let mut catalog = self.catalog.lock();
		catalog.current.audio.push(info.clone());
		catalog.publish()?;

		let producer = TrackProducer::new(producer);
		let consumer = producer.subscribe();

		// Start a task that will remove the catalog on drop.
		let catalog = self.catalog.clone();
		spawn(async move {
			consumer.closed().await.ok();

			let mut catalog = catalog.lock();
			catalog.current.audio.retain(|v| v.track != info.track);
			catalog.publish().unwrap();
		});

		Ok(producer)
	}
}

struct CatalogProducer {
	current: Catalog,
	track: moq_transfork::TrackProducer,
}

impl CatalogProducer {
	fn new(track: moq_transfork::TrackProducer) -> Result<Self> {
		let mut this = Self {
			current: Catalog::default(),
			track,
		};

		// Perform the initial publish
		this.publish()?;

		Ok(this)
	}

	fn publish(&mut self) -> Result<()> {
		let frame = self.current.to_string()?;

		let mut group = self.track.append_group();
		group.write_frame(frame);

		Ok(())
	}
}

// A broadcast consumer, supporting the ability to reload the catalog potentially on a crash.
#[derive(Debug)]
#[debug("{:?}", path)]
pub struct BroadcastConsumer {
	pub session: Session,
	pub path: String,

	// Discovers new broadcasts as they are announced.
	announced: AnnouncedConsumer,

	// The ID of the current broadcast
	current: Option<String>,

	// True if we should None because the broadcast has ended.
	ended: bool,

	catalog_latest: Option<Catalog>,
	catalog_track: Option<moq_transfork::TrackConsumer>,
	catalog_group: Option<moq_transfork::GroupConsumer>,
}

impl BroadcastConsumer {
	pub fn new(session: Session, path: String) -> Self {
		let filter = format!("{}/*/{}", path, ".catalog");
		let announced = session.announced(filter);

		Self {
			session,
			path,
			announced,
			current: None,
			ended: false,
			catalog_latest: None,
			catalog_track: None,
			catalog_group: None,
		}
	}

	pub fn catalog(&self) -> Option<&Catalog> {
		self.catalog_latest.as_ref()
	}

	/// Returns the latest catalog, or None if the channel is offline.
	pub async fn next_catalog(&mut self) -> Result<Option<&Catalog>> {
		loop {
			if self.ended {
				// Avoid returning None again.
				self.ended = false;
				return Ok(None);
			}

			tokio::select! {
				biased;
				// Wait for new announcements.
				Some(announced) = self.announced.next() => {
					// Load or unload based on the announcement.
					match announced {
						Announced::Active(am) => self.load(am),
						Announced::Ended(am) => self.unload(am),
						Announced::Live => {
							// Return None if we're caught up to live with no broadcast.
							if self.current.is_none() {
								return Ok(None)
							}
						},
					}
				},
				Some(group) = async { self.catalog_track.as_mut()?.next_group().await.transpose() } => {
					// Use the new group.
					self.catalog_group.replace(group?);
				},
				Some(frame) = async { self.catalog_group.as_mut()?.read_frame().await.transpose() } => {
					self.catalog_latest = Some(Catalog::from_slice(&frame?)?);
					self.catalog_group.take(); // We don't support deltas yet
					return Ok(self.catalog_latest.as_ref());
				},
				else => return Err(self.session.closed().await.into()),
			}
		}
	}

	fn load(&mut self, am: AnnouncedMatch) {
		let id = am.capture();

		if let Some(current) = &self.current {
			// I'm extremely lazy and using string comparison.
			// This will be wrong when the number of milliseconds since 1970 adds a new digit...
			// But the odds of that happening are low.
			if id <= current.as_str() {
				tracing::warn!(?id, ?current, "ignoring old broadcast");
				return;
			}
		}

		// Make a clone of the match
		let id = id.to_string();
		let path = am.to_full();
		tracing::info!(?path, "loading catalog");

		let track = moq_transfork::Track {
			path,
			priority: -1,
			order: moq_transfork::GroupOrder::Desc,
		};

		self.catalog_track = Some(self.session.subscribe(track));
		self.current = Some(id);
	}

	fn unload(&mut self, am: AnnouncedMatch) {
		if let Some(current) = self.current.as_ref() {
			if current.as_str() == am.capture() {
				self.current = None;
				self.catalog_track = None;
				self.catalog_group = None;
				self.ended = true;
			}
		}
	}

	/// Subscribes to a track
	pub fn track(&self, track: &Track) -> Result<TrackConsumer> {
		let id = self.current.as_ref().ok_or(Error::MissingTrack)?;

		let track = moq_transfork::Track {
			path: format!("{}/{}/{}", self.path, id, &track.name),
			priority: track.priority,

			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		};

		let track = self.session.subscribe(track);
		Ok(TrackConsumer::new(track))
	}
}
