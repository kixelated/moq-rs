use crate::Cluster;

pub struct Connection {
	id: u64,
	session: web_transport::Session,
	cluster: Cluster,
}

impl Connection {
	pub fn new(id: u64, session: web_transport::Session, cluster: Cluster) -> Self {
		Self { id, session, cluster }
	}

	#[tracing::instrument("session", skip_all, err, fields(id = self.id))]
	pub async fn run(self) -> anyhow::Result<()> {
		let session = moq_lite::Session::accept(self.session).await?;
		let mut session1 = session.clone();
		let mut session2 = session.clone();
		let mut session3 = session.clone();

		tokio::select! {
			// Publish any of our broadcasts to the "locals" origin.
			// These are advertised to other nodes in the cluster.
			_ = session1.publish_to(self.cluster.locals.clone(), "") => Ok(()),

			// Consume broadcasts from other clients connected locally.
			_ = session2.consume_from(self.cluster.locals.clone(), "") => Ok(()),

			// Consume broadcasts from other nodes in the cluster.
			_ = session3.consume_from(self.cluster.remotes.clone(), "") => Ok(()),

			// Wait until the session is closed.
			err = session.closed() => Err(err.into()),
		}
	}
}
