use moq_transport::serve::{ServeError, TrackReader};
use url::Url;

mod local;
mod remote;

pub use local::*;
pub use remote::*;

use crate::RelayError;

#[derive(Clone)]
pub struct Origin {
	remotes: Option<(RemotesProducer, RemotesConsumer)>,
	locals: (LocalsProducer, LocalsConsumer),
}

impl Origin {
	// If we have a client, we can fetch remote tracks.
	// If we also have a url, we can serve remote tracks.
	// We can always serve local tracks.
	pub fn new(api: Option<moq_api::Client>, url: Option<Url>, quic: quinn::Endpoint) -> Self {
		let remotes = api.clone().map(|api| Remotes { api, quic }.produce());
		let locals = Locals { api, node: url }.produce();

		Self { remotes, locals }
	}

	pub async fn run(self) -> Result<(), RelayError> {
		if let Some((producer, _)) = self.remotes.as_ref() {
			producer.clone().run().await?;
		}

		Ok(())
	}

	pub fn announce(&mut self, namespace: &str) -> Result<LocalProducer, RelayError> {
		self.locals.0.announce(namespace.to_string())
	}

	pub fn subscribe(&self, namespace: &str, name: &str) -> Result<TrackReader, RelayError> {
		if let Some(local) = self.locals.1.find(namespace) {
			local.subscribe(name)
		} else if let Some((_, remotes)) = self.remotes.as_ref() {
			let remote = remotes.fetch(namespace)?;
			remote.subscribe(name)
		} else {
			Err(ServeError::NotFound.into())
		}
	}
}
