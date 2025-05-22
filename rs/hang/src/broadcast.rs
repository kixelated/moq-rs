use crate::track::TrackConsumer;
use crate::{AudioTrack, Catalog, CatalogConsumer, CatalogProducer, TrackProducer, VideoTrack};
use moq_lite::Track;
use web_async::spawn;

/// A wrapper around a moq_lite::BroadcastProducer that produces a `catalog.json` track.
#[derive(Clone)]
pub struct BroadcastProducer {
	pub catalog: CatalogProducer,
	pub inner: moq_lite::BroadcastProducer,
}

impl Default for BroadcastProducer {
	fn default() -> Self {
		Self::new()
	}
}

impl BroadcastProducer {
	pub fn new() -> Self {
		let catalog = Catalog::default().produce();
		let inner = moq_lite::BroadcastProducer::new();
		inner.insert(catalog.consume().track);

		Self { catalog, inner }
	}

	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer {
			catalog: self.catalog.consume(),
			inner: self.inner.consume(),
		}
	}

	/// Add a video track to the broadcast.
	pub fn add_video(&mut self, track: TrackConsumer, info: VideoTrack) {
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
	pub fn add_audio(&mut self, track: TrackConsumer, info: AudioTrack) {
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

	pub fn create_video(&mut self, video: VideoTrack) -> TrackProducer {
		let producer: TrackProducer = video.track.clone().produce().into();
		self.add_video(producer.consume(), video);
		producer
	}

	pub fn create_audio(&mut self, audio: AudioTrack) -> TrackProducer {
		let producer: TrackProducer = audio.track.clone().produce().into();
		self.add_audio(producer.consume(), audio);
		producer
	}
}

impl std::ops::Deref for BroadcastProducer {
	type Target = moq_lite::BroadcastProducer;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl std::ops::DerefMut for BroadcastProducer {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

/// A wrapper around a moq_lite::BroadcastConsumer that consumes a `catalog.json` track.
#[derive(Clone)]
pub struct BroadcastConsumer {
	pub catalog: CatalogConsumer,
	pub inner: moq_lite::BroadcastConsumer,
}

impl BroadcastConsumer {
	pub fn new(inner: moq_lite::BroadcastConsumer) -> Self {
		let catalog = Track {
			name: Catalog::DEFAULT_NAME.to_string(),
			priority: 0,
		};
		let catalog = inner.subscribe(&catalog).into();

		Self { catalog, inner }
	}

	pub fn track(&self, track: &Track) -> TrackConsumer {
		self.inner.subscribe(track).into()
	}
}

impl std::ops::Deref for BroadcastConsumer {
	type Target = moq_lite::BroadcastConsumer;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl std::ops::DerefMut for BroadcastConsumer {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}
