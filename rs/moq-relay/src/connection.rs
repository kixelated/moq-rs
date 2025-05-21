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
	pub async fn run(self) {
		let session = match moq_lite::Session::accept(self.session).await {
			Ok(session) => session,
			Err(err) => {
				tracing::warn!(?err, "failed to accept session");
				return;
			}
		};

		let mut session1 = session.clone();
		let mut session2 = session.clone();
		let mut session3 = session.clone();

		tokio::select! {
			// Publish any of our broadcasts to the "locals" origin.
			// These are advertised to other nodes in the cluster.
			_ = session1.publish_to(self.cluster.locals.clone(), &self.path) => {},

			// Consume broadcasts from other clients connected locally.
			_ = session2.consume_from(self.cluster.locals.clone(), &self.path) => {},

			// Consume broadcasts from other nodes in the cluster.
			_ = session3.consume_from(self.cluster.remotes.clone(), &self.path) => {},

			// Wait until the session is closed.
			_ = session.closed() => {},
		}
	}
}
