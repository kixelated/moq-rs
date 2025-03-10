use crate::{Audio, Catalog, Error, Result, Track, TrackConsumer, TrackProducer, Video};

use moq_async::{spawn, Lock};
use moq_transfork::{Announced, AnnouncedConsumer, AnnouncedMatch, Session};

use derive_more::Debug;

#[derive(Clone)]
pub struct BroadcastListener {
	pub session: Session,
	pub catalog: Lock<CatalogProducer>,
}
impl BroadcastListener {
	pub fn equals(&self, other: &BroadcastListener) -> bool {
		self.session == other.session
	}
}

#[derive(Debug, Clone)]
struct Context {
	pub video: Option<Video>,
	pub audio: Option<Audio>,
}
impl Context {
	pub fn default() -> Self {
		Self {
			video: None,
			audio: None,
		}
	}

	fn publish_video_to(&self, listener: &mut BroadcastListener, producer: &TrackProducer) -> anyhow::Result<()> {
		if let Some(info) = self.video.clone() {
			let mut catalog = listener.catalog.lock();
			catalog.current.video.push(info.clone());
			catalog.publish()?;

			let consumer = producer.subscribe();
			listener.session.publish(consumer.track.clone())?;

			// Start a task that will remove the catalog on drop.
			let catalog = listener.catalog.clone();
			let track_info = info.track;
			spawn(async move {
				consumer.closed().await.ok();

				let mut catalog = catalog.lock();
				catalog.current.video.retain(|v| v.track != track_info);
				catalog.publish().unwrap();
			});
		}

		Ok(())
	}

	pub fn publish_audio_to(&self, listener: &mut BroadcastListener, producer: &TrackProducer) -> anyhow::Result<()> {
		if let Some(info) = self.audio.clone() {
			let mut catalog = listener.catalog.lock();
			catalog.current.audio.push(info.clone());
			catalog.publish()?;

			let consumer = producer.subscribe();
			listener.session.publish(consumer.track.clone())?;

			// Start a task that will remove the catalog on drop.
			let catalog = listener.catalog.clone();
			let track_info = info.track;
			spawn(async move {
				consumer.closed().await.ok();

				let mut catalog = catalog.lock();
				catalog.current.audio.retain(|v| v.track != track_info);
				catalog.publish().unwrap();
			});
		}

		Ok(())
	}
}

#[derive(Debug, Clone)]
#[debug("{:?}", path)]
pub struct BroadcastProducer {
	listeners: Vec<BroadcastListener>,
	pub path: String,
	id: u64,
	context: Context,
}

impl BroadcastProducer {
	pub fn new(path: String) -> Result<Self> {
		// Generate a "unique" ID for this broadcast session.
		// If we crash, then the viewers will automatically reconnect to the new ID.
		let id = web_time::SystemTime::now()
			.duration_since(web_time::SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64;

		Ok(Self {
			listeners: vec![],
			path,
			id,
			context: Context::default(),
		})
	}

	/// Add a session to the broadcast.
	pub fn add_session(&mut self, mut session: Session, curr_video_producer: Option<&TrackProducer>, curr_audio_producer: Option<&TrackProducer>) -> anyhow::Result<()> {
		let full = format!("{}/{}/catalog.json", self.path, self.id);

		let catalog = moq_transfork::Track {
			path: full,
			priority: -1,
			order: moq_transfork::GroupOrder::Desc,
		}.produce();

		// Publish the catalog track, even if it's empty.
		session.publish(catalog.1)?;

		let catalog = Lock::new(CatalogProducer::new(catalog.0)?);

		let mut listener = BroadcastListener { session, catalog };

		if let Some(producer) = curr_video_producer {
			self.context.publish_video_to(&mut listener, producer)?
		}
		if let Some(producer) = curr_audio_producer {
			self.context.publish_audio_to(&mut listener, producer)?
		}

		self.listeners.push(listener);

		Ok(())
	}

	pub fn publish_video(&mut self, info: Video) -> anyhow::Result<TrackProducer> {
		let path = format!("{}/{}/{}.karp", self.path, self.id, &info.track.name);
		let (producer, _) = moq_transfork::Track {
			path,
			priority: info.track.priority,
			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		}.produce();

		self.context.video = Some(info);
		let producer = TrackProducer::new(producer);

		self.listeners.iter_mut().for_each(|listener| {
			self.context.publish_video_to(listener, &producer).unwrap();
		});

		Ok(producer)
	}

	pub fn publish_audio(&mut self, info: Audio) -> anyhow::Result<TrackProducer> {
		let path = format!("{}/{}/{}.karp", self.path, self.id, &info.track.name);
		let (producer, _) = moq_transfork::Track {
			path,
			priority: info.track.priority,
			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		}.produce();

		self.context.audio = Some(info);
		let producer = TrackProducer::new(producer);

		self.listeners.iter_mut().for_each(|listener| {
			self.context.publish_audio_to(listener, &producer).unwrap();
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
		let filter = moq_transfork::Filter::Wildcard {
			prefix: format!("{}/", path),
			suffix: "/catalog.json".to_string(),
		};
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
			// TODO use the catalog to find the path
			path: format!("{}/{}/{}.karp", self.path, id, &track.name),
			priority: track.priority,

			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		};

		let track = self.session.subscribe(track);
		Ok(TrackConsumer::new(track))
	}
}
