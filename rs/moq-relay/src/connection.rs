use crate::Cluster;

pub struct Connection {
	pub id: u64,
	pub session: web_transport::Session,
	pub cluster: Cluster,
	pub token: moq_token::Payload,
}

impl Connection {
	#[tracing::instrument("conn", skip_all, fields(id = self.id, path = %self.token.path))]
	pub async fn run(mut self) {
		let mut session = match moq_lite::Session::accept(self.session).await {
			Ok(session) => session,
			Err(err) => {
				tracing::warn!(?err, "failed to accept session");
				return;
			}
		};

		// Publish all local and remote broadcasts to the session.
		// TODO We need to learn if this is a relay and NOT publish remotes.
		if let Some(subscribe) = self.token.subscribe {
			let full = format!("{}{}", self.token.path, subscribe);
			let locals = self.cluster.locals.consume_prefix(&full);
			let remotes = self.cluster.remotes.consume_prefix(&full);

			session.publish_prefix(&subscribe, locals);
			session.publish_prefix(&subscribe, remotes);
		}

		// Publish all broadcasts produced by the session to the local origin.
		// TODO These need to be published to remotes if it's a relay.
		if let Some(publish) = self.token.publish {
			let produced = session.consume_prefix(&publish);

			let full = format!("{}{}", self.token.path, publish);
			self.cluster.locals.publish_prefix(&full, produced);
		}

		// Publish this specific broadcast if it's being forced.
		if let Some(publish_force) = self.token.publish_force {
			let produced = session.consume(&publish_force);
			let full = format!("{}{}", self.token.path, publish_force);
			self.cluster.locals.publish(&full, produced);
		}

		// Wait until the session is closed.
		session.closed().await;
	}
}
