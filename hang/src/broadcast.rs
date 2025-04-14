use crate::track::TrackConsumer;
use crate::{Audio, Catalog, CatalogConsumer, CatalogProducer, Result, TrackProducer, Video};
use moq_lite::Track;
use web_async::spawn;

pub use moq_lite::Broadcast;

#[derive(Clone)]
pub struct BroadcastProducer {
	pub catalog: CatalogProducer,
	pub tracks: moq_lite::BroadcastMap,
}

impl BroadcastProducer {
	pub fn new(producer: moq_lite::BroadcastProducer) -> Self {
		let producer = producer.map();

		let catalog = Catalog::default().produce();
		producer.insert(catalog.consume().track);

		Self {
			tracks: producer,
			catalog,
		}
	}

	pub fn consume(&self) -> BroadcastConsumer {
		self.tracks.inner.consume().into()
	}

	pub fn path(&self) -> &str {
		&self.tracks.inner.info.path
	}

	/// Add a video track to the broadcast.
	pub fn add_video(&mut self, track: TrackConsumer, info: Video) {
		self.tracks.insert(track.inner.clone());
		self.catalog.add_video(info.clone());
		self.catalog.publish();

		let mut this = self.clone();
		spawn(async move {
			let _ = track.closed().await;
			this.tracks.remove(&track.inner.info.name);
			this.catalog.remove_video(&info);
			this.catalog.publish();
		});
	}

	/// Add an audio track to the broadcast.
	pub fn add_audio(&mut self, track: TrackConsumer, info: Audio) {
		self.tracks.insert(track.inner.clone());
		self.catalog.add_audio(info.clone());
		self.catalog.publish();

		let mut this = self.clone();
		spawn(async move {
			let _ = track.closed().await;
			this.tracks.remove(&track.inner.info.name);
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

impl From<moq_lite::BroadcastProducer> for BroadcastProducer {
	fn from(inner: moq_lite::BroadcastProducer) -> Self {
		Self::new(inner)
	}
}

#[derive(Clone)]
pub struct BroadcastConsumer {
	pub inner: moq_lite::BroadcastConsumer,
}

impl BroadcastConsumer {
	pub fn new(inner: moq_lite::BroadcastConsumer) -> Self {
		Self { inner }
	}

	pub async fn catalog(&self) -> Result<CatalogConsumer> {
		let track = Track {
			name: Catalog::DEFAULT_NAME.to_string(),
			priority: -1,
		};
		Ok(self.inner.request(track).await?.into())
	}

	/// Subscribes to a track
	pub async fn track(&self, track: Track) -> Result<TrackConsumer> {
		let track = self.inner.request(track).await?;
		Ok(TrackConsumer::new(track))
	}
}

impl From<moq_lite::BroadcastConsumer> for BroadcastConsumer {
	fn from(inner: moq_lite::BroadcastConsumer) -> Self {
		Self::new(inner)
	}
}
