use futures::{stream::FuturesUnordered, StreamExt};
use moq_transfork::{Closed, Publisher, SessionError, Track, TrackReader};

use crate::Locals;

#[derive(Clone)]
pub struct Producer {
	remote: Publisher,
	locals: Locals,
}

impl Producer {
	pub fn new(remote: Publisher, locals: Locals) -> Self {
		Self { remote, locals }
	}

	/*
	pub fn announce(&mut self, tracks: BroadcastReader) -> Result<Announce, SessionError> {
		self.remote.announce(tracks)
	}
	*/

	pub async fn run(mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();
		let mut unknown = self.remote.unknown();

		loop {
			tokio::select! {
				Some(request) = unknown.requested() => {
					tracing::info!("got unknown request");
					let this = self.clone();
					tasks.push(async move {
						match this.route(&request.track).await {
							Ok(track) => request.respond(track),
							Err(err) => request.close(err),
						}
					})
			},
				_= tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn route(&self, track: &Track) -> Result<TrackReader, Closed> {
		if let Some(mut broadcast) = self.locals.route(&track.broadcast) {
			return broadcast.subscribe(track.clone()).await;
		}

		/*

		if let Some(remotes) = &self.remotes {
			if let Some(remote) = remotes.route(&track.broadcast).await {
				return remote.subscribe(&request.broadcast, &request.track);
			}
		}
		*/

		Err(Closed::UnknownBroadcast)
	}
}
