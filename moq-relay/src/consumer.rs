use anyhow::Context;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::{
	serve::{Broadcast, Unknown},
	session::{Announced, SessionError, Subscriber},
};

use crate::{Api, Locals, Producer};

#[derive(Clone)]
pub struct Consumer {
	remote: Subscriber,
	locals: Locals,
	api: Option<Api>,
	forward: Option<Producer>, // Forward all announcements to this subscriber
}

impl Consumer {
	pub fn new(remote: Subscriber, locals: Locals, api: Option<Api>, forward: Option<Producer>) -> Self {
		Self {
			remote,
			locals,
			api,
			forward,
		}
	}

	pub async fn run(mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = self.remote.announced() => {
					let this = self.clone();

					tasks.push(async move {
						let name = announce.broadcast.clone();
						log::info!("serving announce: {:?}", name);

						if let Err(err) = this.serve(announce).await {
							log::warn!("failed serving announce: {:?}, error: {}", name, err)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn serve(mut self, mut announce: Announced) -> Result<(), anyhow::Error> {
		let mut tasks = FuturesUnordered::new();

		let (mut unknown, reader) = Unknown::produce();

		if let Some(api) = self.api.as_ref() {
			let mut refresh = api.set_origin(announce.broadcast.clone()).await?;
			tasks.push(async move { refresh.run().await.context("failed refreshing origin") }.boxed());
		}

		// Register the local tracks, unregister on drop
		let _register = self.locals.register(&announce.broadcast, reader)?;

		// Acknowledge the announce
		announce.ack()?;

		if let Some(mut forward) = self.forward {
			// Make an empty broadcast to forward the announcea
			// TODO this is a which means subscribe won't return any tracks
			let broadcast = Broadcast::new(&announce.broadcast);

			tasks.push(
				async move {
					log::info!("forwarding announce: {:?}", broadcast.name);
					let (_writer, reader) = broadcast.produce();

					let mut announce = forward.announce(reader).context("failed forwarding announce")?;
					announce.closed().await.context("failed forwarding announce")
				}
				.boxed(),
			);
		}

		loop {
			tokio::select! {
				// If the announce is closed, return the error
				Err(err) = announce.closed() => return Err(err.into()),

				// Wait for the next subscriber and serve the track.
				Some(request) = unknown.requested() => {
					let mut remote = self.remote.clone();

					tasks.push(async move {
						log::info!("forwarding subscribe: {:?}", request.track);
						let writer = request.produce();

						if let Ok(sub) = remote.subscribe(writer) {
							sub.closed().await;
						};

						Ok(())
					}.boxed());
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
				else => return Ok(()),
			}
		}
	}
}
