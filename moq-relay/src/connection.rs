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

	#[tracing::instrument("connection", skip_all, err, fields(id = self.id))]
	pub async fn run(mut self) -> anyhow::Result<()> {
		let mut session = moq_transfork::Session::accept(self.session).await?;

		// Route any subscriptions to the cluster
		session.route(self.cluster.router());
		session.announce(self.cluster.announced());
		self.cluster.announce(session.announced(), session.clone());

		session.closed().await;

		Ok(())
	}
}
