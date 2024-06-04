use anyhow::Context;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::{
	serve::Broadcast,
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
						let info = announce.clone();
						log::info!("serving announce: {:?}", info);

						if let Err(err) = this.serve(announce).await {
							log::warn!("failed serving announce: {:?}, error: {}", info, err)
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

		let (_, mut request, broadcast) = Broadcast::new(announce.broadcast.to_string()).produce();

		if let Some(api) = self.api.as_ref() {
			let mut refresh = api.set_origin(broadcast.name.clone()).await?;
			tasks.push(async move { refresh.run().await.context("failed refreshing origin") }.boxed());
		}

		// Register the local tracks, unregister on drop
		let _register = self.locals.register(broadcast.clone()).await?;

		announce.ok()?;

		if let Some(mut forward) = self.forward {
			tasks.push(
				async move {
					log::info!("forwarding announce: {:?}", broadcast.info);
					let announce = forward.announce(broadcast).context("failed forwarding announce")?;
					announce.closed().await
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
					let mut remote = self.remote.clone();

					tasks.push(async move {
						let info = track.clone();
						log::info!("forwarding subscribe: {:?}", info);

						if let Ok(sub) = remote.subscribe(&announce.name, track) {
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
