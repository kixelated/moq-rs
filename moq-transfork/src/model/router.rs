use std::ops;

use tokio::sync::{mpsc, oneshot};

use crate::{Error, Track, TrackConsumer, TrackProducer};

/// Used to respond to arbitrary track requests.
pub struct Router {
	/// The maximum number of requests that can be queued before blocking.
	pub capacity: usize,
}

impl Router {
	pub fn produce(&self) -> (RouterProducer, RouterConsumer) {
		let (send, recv) = mpsc::channel(self.capacity);

		let writer = RouterProducer::new(recv);
		let reader = RouterConsumer::new(send);

		(writer, reader)
	}
}

impl Default for Router {
	fn default() -> Self {
		Self { capacity: 32 }
	}
}

/// Receive broadcast/track requests and return if we can fulfill them.
pub struct RouterProducer {
	queue: mpsc::Receiver<RouterRequest>,
}

impl RouterProducer {
	fn new(queue: mpsc::Receiver<RouterRequest>) -> Self {
		Self { queue }
	}

	pub async fn requested(&mut self) -> Option<RouterRequest> {
		self.queue.recv().await
	}
}

/// Subscribe to abitrary broadcast/tracks.
#[derive(Clone)]
pub struct RouterConsumer {
	queue: mpsc::Sender<RouterRequest>,
}

impl RouterConsumer {
	fn new(queue: mpsc::Sender<RouterRequest>) -> Self {
		Self { queue }
	}

	pub async fn subscribe(&self, track: Track) -> Result<TrackConsumer, Error> {
		let (send, recv) = oneshot::channel();
		let request = RouterRequest { track, reply: send };

		if self.queue.send(request).await.is_err() {
			return Err(Error::Cancel);
		}

		recv.await.map_err(|_| Error::Cancel)?
	}

	pub async fn closed(&self) {
		self.queue.closed().await;
	}
}

/// An outstanding request for a path.
pub struct RouterRequest {
	pub track: Track,
	reply: oneshot::Sender<Result<TrackConsumer, Error>>,
}

impl RouterRequest {
	pub fn serve(self, reader: TrackConsumer) {
		self.reply.send(Ok(reader)).ok();
	}

	pub fn produce(self) -> TrackProducer {
		let (writer, reader) = self.track.produce();
		self.reply.send(Ok(reader)).ok();
		writer
	}

	pub fn close(self, error: Error) {
		self.reply.send(Err(error)).ok();
	}
}

impl ops::Deref for RouterRequest {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}
