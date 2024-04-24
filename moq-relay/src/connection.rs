use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::{
	serve::ServeError,
	session::{Announced, Publisher, SessionError, Subscribed, Subscriber},
};

use crate::{error::RelayError, Locals, RemotesConsumer};

#[derive(Clone)]
pub struct Connection {
	locals: Locals,
	remotes: Option<RemotesConsumer>,
}

impl Connection {
	pub fn new(locals: Locals, remotes: Option<RemotesConsumer>) -> Self {
		Self { locals, remotes }
	}

	pub async fn run(self, conn: web_transport_quinn::Session) -> anyhow::Result<()> {
		let (session, publisher, subscriber) = moq_transport::session::Session::accept(conn.into()).await?;

		let mut tasks = FuturesUnordered::new();
		tasks.push(session.run().boxed_local());

		if let Some(remote) = publisher {
			tasks.push(Self::serve_subscriber(self.clone(), remote).boxed_local());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(self.clone(), remote).boxed_local());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_subscriber(self, mut remote: Publisher) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(subscribe) = remote.subscribed() => {
					let conn = self.clone();

					tasks.push(async move {
						let info = subscribe.info.clone();
						log::info!("serving subscribe: {:?}", info);

						if let Err(err) = conn.serve_subscribe(subscribe).await {
							log::warn!("failed serving subscribe: {:?}, error: {}", info, err)
						}
					})
				},
				_= tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn serve_subscribe(self, subscribe: Subscribed) -> Result<(), RelayError> {
		if let Some(mut local) = self.locals.route(&subscribe.namespace) {
			log::debug!("using local announce: {:?}", local.info);
			if let Some(track) = local.subscribe(&subscribe.name) {
				log::info!("serving from local: {:?}", track.info);
				// NOTE: Depends on drop(track) being called afterwards
				return Ok(subscribe.serve(track).await?);
			}
		}

		if let Some(remotes) = &self.remotes {
			if let Some(remote) = remotes.route(&subscribe.namespace).await? {
				log::debug!("using remote announce: {:?}", remote.info);
				if let Some(track) = remote.subscribe(subscribe.namespace.clone(), subscribe.name.clone())? {
					log::info!("serving from remote: {:?} {:?}", remote.info, track.info);

					// NOTE: Depends on drop(track) being called afterwards
					return Ok(subscribe.serve(track.reader).await?);
				}
			}
		}

		Err(ServeError::NotFound.into())
	}

	async fn serve_publisher(self, mut remote: Subscriber) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = remote.announced() => {
					let remote = remote.clone();
					let conn = self.clone();

					tasks.push(async move {
						let info = announce.info.clone();
						log::info!("serving announce: {:?}", info);

						if let Err(err) = conn.serve_announce(remote, announce).await {
							log::warn!("failed serving announce: {:?}, error: {}", info, err)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
			};
		}
	}

	async fn serve_announce(mut self, remote: Subscriber, mut announce: Announced) -> Result<(), RelayError> {
		let mut publisher = match self.locals.announce(announce.namespace).await {
			Ok(publisher) => {
				announce.ok()?;
				publisher
			}
			Err(err) => {
				// TODO use better error codes
				announce.close(err.clone().into())?;
				return Err(err);
			}
		};

		let mut tasks = FuturesUnordered::new();

		let mut done = None;

		loop {
			tokio::select! {
				// If the announce is closed, return the error
				res = announce.closed(), if done.is_none() => done = Some(res),

				// Wait for the next subscriber and serve the track.
				res = publisher.requested(), if done.is_none() => {
					let track = match res? {
						Some(track) => track,
						None => {
							done = Some(Ok(()));
							continue
						},
					};

					let mut subscriber = remote.clone();

					tasks.push(async move {
						let info = track.info.clone();
						log::info!("relaying track: track={:?}", info);

						if let Err(err) = subscriber.subscribe(track).closed().await {
							log::warn!("failed serving track: {:?}, error: {}", info, err);
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {}

				// Done must be set and there are no tasks left
				else => return Ok(done.unwrap()?),
			}
		}
	}
}
