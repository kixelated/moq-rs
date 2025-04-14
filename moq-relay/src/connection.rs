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
		let session = moq_lite::Session::accept(self.session).await?;

		let locals = self.cluster.locals.clone();

		// TODO There will be errors if locals and remotes announce the same path.
		tokio::select! {
			res = locals.publish_to(session.clone()) => res,
			res = self.cluster.remotes.publish_to(session.clone()) => res,
			_ = self.cluster.locals.subscribe_from(session.clone()) => Ok(()),
			err = session.closed() => Err(err.into()),
		}
	}
}
