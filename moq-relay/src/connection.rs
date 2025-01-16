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
	pub async fn run(mut self) -> anyhow::Result<()> {
		let mut session = moq_transfork::Session::accept(self.session).await?;

		// Route any subscriptions to the cluster
		session.route(self.cluster.router);

		// TODO things will get weird if locals and remotes announce the same path.
		session.announce(self.cluster.locals.announced());
		session.announce(self.cluster.remotes.announced());

		// Add any announcements to the cluster, indicating we're the origin.
		let all = session.announced(moq_transfork::Path::default());
		self.cluster.locals.announce(all, Some(session.clone())).await;

		Ok(())
	}
}
