use futures::{stream::FuturesUnordered, StreamExt};
use moq_transfork::{Publisher, SessionError, Track, TrackReader};

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
		log::info!("running producer");

		let mut tasks = FuturesUnordered::new();
		let mut unknown = self.remote.unknown();

		loop {
			tokio::select! {
				Some(request) = unknown.requested() => {
					log::info!("got unknown request");
					let this = self.clone();
					tasks.push(async move {
						if let Some(track) = this.route(&request.track).await {
							request.respond(track);
						}
					})
			},
				_= tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn route(&self, track: &Track) -> Option<TrackReader> {
		log::info!("routing track: {:?}", track);

		if let Some(mut broadcast) = self.locals.route(&track.broadcast) {
			log::info!("found: {:?}", broadcast.info);
			return broadcast.subscribe(track.clone()).await;
		}

		log::info!("did not find");

		/*

		if let Some(remotes) = &self.remotes {
			if let Some(remote) = remotes.route(&track.broadcast).await {
				return remote.subscribe(&request.broadcast, &request.track);
			}
		}
		*/

		None
	}
}
