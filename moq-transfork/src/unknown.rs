use std::ops;

use crate::{
	util::{Queue, State},
	TrackWriter,
};

use super::{Track, TrackReader};

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

	pub async fn subscribe(&self, track: Track) -> Option<TrackReader> {
		let request = UnknownRequest::new(track);
		if self.queue.push(request.split()).is_err() {
			return None;
		}

		request.response().await
	}
}

pub struct UnknownRequest {
	pub track: Track,
	reply: State<Option<TrackReader>>,
}

impl UnknownRequest {
	fn new(track: Track) -> Self {
		Self {
			track,
			reply: State::default(),
		}
	}

	fn split(&self) -> Self {
		Self {
			track: self.track.clone(),
			reply: self.reply.split(),
		}
	}

	pub fn respond(self, track: TrackReader) {
		if let Some(mut state) = self.reply.lock_mut() {
			state.replace(track);
		}
	}

	pub fn produce(self) -> TrackWriter {
		// TODO avoid this clone
		let (writer, reader) = self.track.clone().produce();
		self.respond(reader);
		writer
	}

	pub async fn response(self) -> Option<TrackReader> {
		loop {
			{
				let state = self.reply.lock();
				if let Some(track) = state.clone() {
					return Some(track);
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
