use anyhow::Context;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::{
	serve::{ServeError, Tracks},
	session::{Announced, Publisher, SessionError, Subscribed, Subscriber},
};

use crate::{Locals, RemotesConsumer};

#[derive(Clone)]
pub struct Session {
	session: web_transport_quinn::Session,
	locals: Locals,
	remotes: Option<RemotesConsumer>,
	forward: Option<Publisher>, // Forward all announcements to this publisher
}

impl Session {
	pub fn new(
		session: web_transport_quinn::Session,
		locals: Locals,
		remotes: Option<RemotesConsumer>,
		forward: Option<Publisher>,
	) -> Self {
		Self {
			session,
			locals,
			remotes,
			forward,
		}
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let session = self.session.clone().into();
		let (session, publisher, subscriber) = moq_transport::session::Session::accept(session).await?;

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
						let info = subscribe.clone();
						log::info!("serving subscribe: {:?}", info);

						if let Err(err) = conn.serve_subscribe(subscribe).await {
							log::warn!("failed serving subscribe: {:?}, error: {}", info, err)
						}
					})
				},
				_= tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn serve_subscribe(self, subscribe: Subscribed) -> Result<(), anyhow::Error> {
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

	async fn serve_publisher(self, mut remote: Subscriber) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = remote.announced() => {
					let remote = remote.clone();
					let conn = self.clone();

					tasks.push(async move {
						let info = announce.clone();
						log::info!("serving announce: {:?}", info);

						if let Err(err) = conn.serve_announce(remote, announce).await {
							log::warn!("failed serving announce: {:?}, error: {}", info, err)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn serve_announce(mut self, remote: Subscriber, mut announce: Announced) -> Result<(), anyhow::Error> {
		let mut tasks = FuturesUnordered::new();

		let (_, mut request, reader) = Tracks::new(announce.namespace.to_string()).produce();

		let register = self.locals.register(reader.clone()).await?;
		tasks.push(register.run().boxed());

		announce.ok()?;

		if let Some(mut forward) = self.forward {
			tasks.push(
				async move {
					log::info!("forwarding announce: {:?}", reader.info);
					forward.announce(reader).await.context("failed forwarding announce")
				}
				.boxed(),
			);
		}

		loop {
			tokio::select! {
				// If the announce is closed, return the error
				Err(err) = announce.closed() => return Err(err.into()),

				// Wait for the next subscriber and serve the track.
				Some(track) = request.next() => {
					let mut subscriber = remote.clone();

					tasks.push(async move {
						let info = track.clone();
						log::info!("forwarding subscribe: {:?}", info);

						if let Err(err) = subscriber.subscribe(track).await {
							log::warn!("failed forwarding subscribe: {:?}, error: {}", info, err)
						}

						Ok(())
					}.boxed());
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
				else => return Ok(()),
			}
		}
	}
}
