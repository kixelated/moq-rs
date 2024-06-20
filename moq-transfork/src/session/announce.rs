use crate::{
	coding::{self},
	BroadcastReader,
};

use super::SessionError;

pub struct Announce {
	pub broadcast: String,
	reader: BroadcastReader,
	stream: coding::Stream,
}

impl Announce {
	pub fn new(reader: BroadcastReader, stream: coding::Stream) -> Self {
		Self {
			broadcast: reader.name.clone(),
			reader,
			stream,
		}
	}

	pub async fn run(&mut self) {
		let res = tokio::select! {
			res = self.stream.reader.closed() => res.map_err(SessionError::from),
			res = self.reader.closed() => res.map_err(SessionError::from),
		};

		if let Err(err) = res {
			self.stream.writer.reset(err.code());
		}
	}
}
