use crate::track::TrackConsumer;
use crate::{Audio, Catalog, CatalogConsumer, CatalogProducer, TrackProducer, Video};
use moq_lite::Track;
use web_async::spawn;

pub use moq_lite::Broadcast;

#[derive(Clone)]
pub struct BroadcastProducer {
	pub catalog: CatalogProducer,
	pub producer: moq_lite::BroadcastProducer,
}

impl BroadcastProducer {
	pub fn new(producer: moq_lite::BroadcastProducer) -> Self {
		let catalog = Catalog::default().produce();
		producer.insert(catalog.consume().track);

		Self { producer, catalog }
	}

	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer::new(self.producer.consume())
	}

	pub fn path(&self) -> &str {
		&self.producer.info.path
	}

	/// Add a video track to the broadcast.
	pub fn add_video(&mut self, track: TrackConsumer, info: Video) {
		self.producer.insert(track.inner.clone());
		self.catalog.add_video(info.clone());
		self.catalog.publish();

		let mut this = self.clone();
		spawn(async move {
			let _ = track.closed().await;
			this.producer.remove(&track.inner.info.name);
			this.catalog.remove_video(&info);
			this.catalog.publish();
		});
	}

	/// Add an audio track to the broadcast.
	pub fn add_audio(&mut self, track: TrackConsumer, info: Audio) {
		self.producer.insert(track.inner.clone());
		self.catalog.add_audio(info.clone());
		self.catalog.publish();

		let mut this = self.clone();
		spawn(async move {
			let _ = track.closed().await;
			this.producer.remove(&track.inner.info.name);
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

/*
impl From<moq_lite::BroadcastProducer> for BroadcastProducer {
	fn from(inner: moq_lite::BroadcastProducer) -> Self {
		Self::new(inner)
	}
}
*/

#[derive(Clone)]
pub struct BroadcastConsumer {
	pub inner: moq_lite::BroadcastConsumer,
	pub catalog: CatalogConsumer,
}

impl BroadcastConsumer {
	pub fn new(inner: moq_lite::BroadcastConsumer) -> Self {
		let track = Track {
			name: Catalog::DEFAULT_NAME.to_string(),
			priority: -1,
		};

		let catalog = inner.subscribe(&track).into();
		Self { inner, catalog }
	}

	/// Subscribes to a track
	pub fn track(&self, track: &Track) -> TrackConsumer {
		self.inner.subscribe(track).into()
	}
}

/* Disabled so it's more clear that we're wrapping a moq_lite::BroadcastConsumer.
impl From<moq_lite::BroadcastConsumer> for BroadcastConsumer {
	fn from(inner: moq_lite::BroadcastConsumer) -> Self {
		Self::new(inner)
	}
}
*/
