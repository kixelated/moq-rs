use crate::track::TrackConsumer;
use crate::{Audio, Catalog, Error, Result, Track, TrackProducer, Video};
use derive_more::Debug;
use moq_async::Lock;
use moq_transfork::{Announced, AnnouncedConsumer, AnnouncedMatch, Session};

struct BroadcastProducerState {
	catalog: CatalogProducer,
	tracks: Vec<moq_transfork::TrackProducer>,
	subscribers: Vec<Session>,
}

#[derive(Debug, Clone)]
#[debug("{:?}", path)]
pub struct BroadcastProducer {
	pub path: String,
	id: u64,
	state: Lock<BroadcastProducerState>,
}

impl BroadcastProducer {
	pub fn new(path: String) -> Result<Self> {
		// Generate a "unique" ID for this broadcast session.
		// If we crash, then the viewers will automatically reconnect to the new ID.
		let id = web_time::SystemTime::now()
			.duration_since(web_time::SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64;

		// Create the catalog track
		let full = format!("{}/{}/catalog.json", path.clone(), id.clone());
		let (catalog, _) = moq_transfork::Track {
			path: full,
			priority: -1,
			order: moq_transfork::GroupOrder::Desc,
		}
		.produce();
		let catalog = CatalogProducer::new(catalog)?;

		// Create the BroadcastProducerState
		let state = Lock::new(BroadcastProducerState {
			catalog,
			tracks: vec![],
			subscribers: vec![],
		});

		Ok(Self { path, id, state })
	}

	/// Add a session to the broadcast.
	/// If the session closes, it will be removed from the broadcast automatically.
	pub fn add_session(&mut self, mut session: Session) -> Result<()> {
		let mut state = self.state.lock();

		// Publish the catalog
		session.publish(state.catalog.track.subscribe())?;

		// Publish all tracks
		for track in &state.tracks {
			tracing::info!("publishing track, {:?}", track.path);
			session.publish(track.subscribe())?;
		}

		// Add the session to the list of subscribers
		state.subscribers.push(session.clone());

		// If the session closes, remove it from the list of subscribers
		let state = self.state.clone();
		moq_async::spawn(async move {
			session.closed().await;
			state.lock().subscribers.retain(|s| s != &session);
		});

		Ok(())
	}

	/// Remove a session from the broadcast.
	pub fn remove_session(&mut self, session: &Session) {
		let mut state = self.state.lock();
		state.subscribers.retain(|s| s != session);
	}

	/// Publish a video track to all listeners & future listeners.
	pub fn publish_video(&mut self, info: Video) -> Result<TrackProducer> {
		let mut state = self.state.lock();

		let producer = self.publish(info.track.clone(), &mut state)?;
		state.catalog.current.video.push(info);
		state.catalog.publish()?;

		Ok(producer)
	}

	/// Publish an audio track to all listeners & future listeners.
	pub fn publish_audio(&mut self, info: Audio) -> Result<TrackProducer> {
		let mut state = self.state.lock();

		let producer = self.publish(info.track.clone(), &mut state)?;
		state.catalog.current.audio.push(info);
		state.catalog.publish()?;

		Ok(producer)
	}

	fn publish(&self, track: Track, state: &mut BroadcastProducerState) -> Result<TrackProducer> {
		// Create the TrackProducer
		let path = format!("{}/{}/{}.karp", self.path, self.id, &track.name);
		let (producer, _) = moq_transfork::Track {
			path,
			priority: track.priority,
			// TODO add these to the catalog and support higher latencies.
			order: moq_transfork::GroupOrder::Desc,
		}
		.produce();

		// Let all the listeners know about the new video track
		state.subscribers.iter_mut().for_each(|session| {
			session.publish(producer.subscribe()).unwrap();
		});

		// Update the state
		state.tracks.push(producer.clone());

		// Return the producer
		let producer = TrackProducer::new(producer);
		Ok(producer)
	}
}

#[derive(Debug, Clone)]
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
