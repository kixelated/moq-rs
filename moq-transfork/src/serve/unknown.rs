use crate::util::{Queue, State};

use super::TrackReader;

#[derive(Default)]
pub struct Unknown {}

impl Unknown {
	pub fn produce() -> (UnknownWriter, UnknownReader) {
		let queue = Queue::default();

		let writer = UnknownWriter::new(queue.split());
		let reader = UnknownReader::new(queue);

		(writer, reader)
	}
}

pub struct UnknownWriter {
	queue: Queue<UnknownRequest>,
}

impl UnknownWriter {
	fn new(queue: Queue<UnknownRequest>) -> Self {
		Self { queue }
	}

	pub async fn requested(&mut self) -> Option<UnknownRequest> {
		self.queue.pop().await
	}
}

#[derive(Clone)]
pub struct UnknownReader {
	queue: Queue<UnknownRequest>,
}

impl UnknownReader {
	fn new(queue: Queue<UnknownRequest>) -> Self {
		Self { queue }
	}

	pub async fn request(&mut self, broadcast: &str, track: &str) -> Option<TrackReader> {
		let request = UnknownRequest::new(broadcast, track);
		if self.queue.push(request.split()).is_err() {
			return None;
		}

		request.response().await
	}
}

pub struct UnknownRequest {
	pub broadcast: String,
	pub track: String,
	state: State<Option<TrackReader>>,
}

impl UnknownRequest {
	fn new(broadcast: &str, track: &str) -> Self {
		Self {
			broadcast: broadcast.to_string(),
			track: track.to_string(),
			state: Default::default(),
		}
	}

	fn split(&self) -> Self {
		Self {
			broadcast: self.broadcast.clone(),
			track: self.track.clone(),
			state: self.state.split(),
		}
	}

	pub async fn respond(self, reader: TrackReader) {
		if let Some(mut state) = self.state.lock_mut() {
			state.replace(reader);
		}
	}

	async fn response(&self) -> Option<TrackReader> {
		{
			let state = self.state.lock();
			if state.is_some() {
				return state.clone();
			}

			state.modified()?
		}
		.await;

		self.state.lock().clone()
	}
}
