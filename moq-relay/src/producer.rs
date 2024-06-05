use futures::{stream::FuturesUnordered, StreamExt};
use moq_transfork::{
	serve::{BroadcastReader, ServeError, Track, TrackReader, UnknownRequest},
	session::{Announce, Publisher, SessionError, Subscribed},
};

use crate::{Locals, RemotesConsumer};

#[derive(Clone)]
pub struct Producer {
	remote: Publisher,
	locals: Locals,
	remotes: Option<RemotesConsumer>,
}

impl Producer {
	pub fn new(remote: Publisher, locals: Locals, remotes: Option<RemotesConsumer>) -> Self {
		Self {
			remote,
			locals,
			remotes,
		}
	}

	pub fn announce(&mut self, tracks: BroadcastReader) -> Result<Announce, SessionError> {
		self.remote.announce(tracks)
	}

	pub async fn run(mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut unknown = self.remote.unknown().unwrap();

		loop {
			tokio::select! {
				Some(request) = unknown.requested() => tasks.push(async move {
					if let Some(track) = self.route(&request.track).await {
						request.respond(track);
					}
				}),
				_= tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn route(&self, track: &Track) -> Option<TrackReader> {
		if let Some(mut broadcast) = self.locals.route(&track.broadcast) {
			return broadcast.request(track.clone()).await;
		}

		if let Some(remotes) = &self.remotes {
			if let Some(remote) = remotes.route(&track.broadcast).await {
				return remote.subscribe(&request.broadcast, &request.track);
			}
		}

		None
	}
}
