use crate::track::TrackConsumer;
use crate::{Audio, Catalog, CatalogConsumer, CatalogProducer, TrackProducer, Video};
use moq_lite::Track;
use web_async::spawn;

/// A wrapper around [moq_lite::Broadcast], with a room and name.
/// .hang is appended to the end when publishing.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Broadcast {
	pub room: String,
	pub name: String,
}

impl Broadcast {
	pub fn produce(self) -> BroadcastProducer {
		BroadcastProducer::new(self)
	}
}

// Convert to a moq_lite::Broadcast.
impl From<Broadcast> for moq_lite::Broadcast {
	fn from(broadcast: Broadcast) -> Self {
		moq_lite::Broadcast {
			path: format!("{}/{}.hang", broadcast.room, broadcast.name),
		}
	}
}

#[derive(Clone)]
pub struct BroadcastProducer {
	pub info: Broadcast,
	pub catalog: CatalogProducer,
	pub inner: moq_lite::BroadcastProducer,
}

impl BroadcastProducer {
	pub fn new(info: Broadcast) -> Self {
		let catalog = Catalog::default().produce();

		let inner = moq_lite::BroadcastProducer::new(info.clone().into());
		inner.insert(catalog.consume().track);

		Self { info, catalog, inner }
	}

	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer {
			info: self.info.clone(),
			catalog: self.catalog.consume(),
			inner: self.inner.consume(),
		}
	}

	pub fn name(&self) -> &str {
		&self.info.name
	}

	/// Add a video track to the broadcast.
	pub fn add_video(&mut self, track: TrackConsumer, info: Video) {
		self.inner.insert(track.inner.clone());
		self.catalog.add_video(info.clone());
		self.catalog.publish();

		let mut this = self.clone();
		spawn(async move {
			let _ = track.closed().await;
			this.inner.remove(&track.inner.info.name);
			this.catalog.remove_video(&info);
			this.catalog.publish();
		});
	}

	/// Add an audio track to the broadcast.
	pub fn add_audio(&mut self, track: TrackConsumer, info: Audio) {
		self.inner.insert(track.inner.clone());
		self.catalog.add_audio(info.clone());
		self.catalog.publish();

		let mut this = self.clone();
		spawn(async move {
			let _ = track.closed().await;
			this.inner.remove(&track.inner.info.name);
			this.catalog.remove_audio(&info);
			this.catalog.publish();
		});
	}

	pub fn create_video(&mut self, video: Video) -> TrackProducer {
		let producer: TrackProducer = video.track.clone().produce().into();
		self.add_video(producer.consume(), video);
		producer
	}

	pub fn create_audio(&mut self, audio: Audio) -> TrackProducer {
		let producer: TrackProducer = audio.track.clone().produce().into();
		self.add_audio(producer.consume(), audio);
		producer
	}
}

#[derive(Clone)]
pub struct BroadcastConsumer {
	pub info: Broadcast,
	pub catalog: CatalogConsumer,
	pub inner: moq_lite::BroadcastConsumer,
}

impl BroadcastConsumer {
	pub fn track(&self, track: &Track) -> TrackConsumer {
		self.inner.subscribe(track).into()
	}

	pub fn subscribe(session: &moq_lite::Session, info: Broadcast) -> BroadcastConsumer {
		let consumer = session.consume(&info.clone().into());
		let catalog = Track {
			name: Catalog::DEFAULT_NAME.to_string(),
			priority: 0,
		};
		let catalog = consumer.subscribe(&catalog);

		BroadcastConsumer {
			info,
			catalog: catalog.into(),
			inner: consumer,
		}
	}
}
