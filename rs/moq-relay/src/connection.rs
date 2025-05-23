use crate::Cluster;

pub struct Connection {
	id: u64,
	session: web_transport::Session,
	cluster: Cluster,
	path: String,
}

impl Connection {
	pub fn new(id: u64, session: web_transport::Session, cluster: Cluster) -> Self {
		// Scope everything to the session URL path.
		// ex. if somebody connects with `/foo/bar/` then SUBSCRIBE "baz" will return `/foo/bar/baz`.
		// TODO sign this path so it can't be modified by an unauthenticated user.
		let path = session.url().path().strip_prefix("/").unwrap_or("").to_string();

		Self {
			id,
			session,
			cluster,
			path,
		}
	}

	#[tracing::instrument("session", skip_all, fields(id = self.id, path = self.path))]
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
		let locals = self.cluster.locals.consume_prefix(&self.path);
		let remotes = self.cluster.remotes.consume_prefix(&self.path);
		session.publish_all(locals);
		session.publish_all(remotes);

		// Publish all broadcasts produced by the session to the local origin.
		// TODO These need to be published to remotes if it's a relay.
		let produced = session.consume_all();
		self.cluster.locals.publish_all(produced);

		// Wait until the session is closed.
		session.closed().await;
	}
}
