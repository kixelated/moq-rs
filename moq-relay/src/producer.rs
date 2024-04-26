use futures::{stream::FuturesUnordered, StreamExt};
use moq_transport::{
	serve::{ServeError, TracksReader},
	session::{Publisher, SessionError, Subscribed},
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

	pub async fn announce(&mut self, tracks: TracksReader) -> Result<(), SessionError> {
		self.remote.announce(tracks).await
	}

	pub async fn run(mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(subscribe) = self.remote.subscribed() => {
					let this = self.clone();

					tasks.push(async move {
						let info = subscribe.clone();
						log::info!("serving subscribe: {:?}", info);

						if let Err(err) = this.serve(subscribe).await {
							log::warn!("failed serving subscribe: {:?}, error: {}", info, err)
						}
					})
				},
				_= tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn serve(self, subscribe: Subscribed) -> Result<(), anyhow::Error> {
		if let Some(mut local) = self.locals.route(&subscribe.namespace) {
			if let Some(track) = local.subscribe(&subscribe.name) {
				log::info!("serving from local: {:?}", track.info);
				return Ok(subscribe.serve(track).await?);
			}
		}

		if let Some(remotes) = &self.remotes {
			if let Some(remote) = remotes.route(&subscribe.namespace).await? {
				if let Some(track) = remote.subscribe(subscribe.namespace.clone(), subscribe.name.clone())? {
					log::info!("serving from remote: {:?} {:?}", remote.info, track.info);

					// NOTE: Depends on drop(track) being called afterwards
					return Ok(subscribe.serve(track.reader).await?);
				}
			}
		}

		Err(ServeError::NotFound.into())
	}
}
