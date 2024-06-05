use std::ops;

use crate::util::{Queue, State};

use super::{ServeError, Track, TrackReader, TrackWriter};

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

	pub async fn request(&self, track: Track) -> Option<TrackReader> {
		let request = UnknownRequest::new(track);

		self.queue.push(request.split());
		request.response().await
	}
}

pub struct UnknownRequest {
	pub track: Track,
	state: State<Option<TrackReader>>,
}

impl UnknownRequest {
	fn new(track: Track) -> Self {
		Self {
			track,
			state: Default::default(),
		}
	}

	fn split(&self) -> UnknownRequest {
		Self {
			state: self.state.split(),
			track: self.track.clone(),
		}
	}

	pub fn respond(self, reader: TrackReader) {
		if let Some(mut state) = self.state.lock_mut() {
			state.replace(reader);
		}
	}

	pub fn produce(self) -> TrackWriter {
		let (writer, reader) = self.track.clone().produce();
		self.respond(reader);
		writer
	}

	async fn response(&self) -> Option<TrackReader> {
		loop {
			{
				let state = self.state.lock();
				if state.is_some() {
					return state.clone();
				}
				state.modified()?
			}
			.await
		}
	}
}

impl ops::Deref for UnknownRequest {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}
