use futures::{stream::FuturesUnordered, StreamExt};
use moq_transfork::{BroadcastReader, SessionError, Subscriber};

use crate::Locals;

#[derive(Clone)]
pub struct Consumer {
	remote: Subscriber,
	locals: Locals,
	//forward: Option<Producer>, // Forward all announcements to this subscriber
}

impl Consumer {
	pub fn new(remote: Subscriber, locals: Locals) -> Self {
		Self { remote, locals }
	}

	pub async fn run(mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(broadcast) = self.remote.announced() => {
					let this = self.clone();
					tasks.push(this.serve(broadcast))
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	#[tracing::instrument("serve", skip_all, err, fields(broadcast = broadcast.name))]
	async fn serve(mut self, broadcast: BroadcastReader) -> Result<(), anyhow::Error> {
		//let mut tasks = FuturesUnordered::new();

		// Register the local tracks, unregister on drop
		let _register = self.locals.announce(broadcast.clone());

		broadcast.closed().await?;
		Ok(())

		/*
		if let Some(mut forward) = self.forward {
			// Make an empty broadcast to forward the announcea
			// TODO this is a which means subscribe won't return any tracks
			let broadcast = Broadcast::new(&announce.broadcast);

			tasks.push(
				async move {
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
				Err(err) = broadcast.closed() => return Err(err.into()),

				// Wait for the next subscriber and serve the track.
				Some(request) = unknown.requested() => {
					let mut remote = self.remote.clone();

					tasks.push(async move {
						let writer = request.produce();

						let sub = remote.subscribe(writer);
						sub.closed().await
					}.boxed());
				},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
				else => return Ok(()),
			}
		}
		*/
	}
}
