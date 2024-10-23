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
		session.route(Some(self.cluster.router()));

		let mut upstream = self.cluster.announced();
		let mut downstream = session.announced();

		loop {
			tokio::select! {
				Some(announced) = upstream.next() => {
					match session.announce(announced.path.clone()) {
						Ok(active) => {
							tokio::spawn(async move {
								announced.closed().await;
								drop(active);
							});
						}
						Err(err) => tracing::warn!(?err, "failed announce from upstream"),
					};
				},
				Some(announced) = downstream.next() => {
					if let Err(err) = self.cluster.announce(announced, session.clone()) {
						tracing::warn!(?err, "failed announce from downstream");
					}
				},
				_ = session.closed() => return Ok(()),
			}
		}
	}
}
