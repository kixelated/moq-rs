use moq_transfork::*;
use tracing::Instrument;

pub struct Connection {
	id: u64,
	session: web_transport::Session,

	local: AnnouncedProducer,
	remote: AnnouncedConsumer,
}

impl Connection {
	pub fn new(id: u64, session: web_transport::Session, local: AnnouncedProducer, remote: AnnouncedConsumer) -> Self {
		Self {
			id,
			session,
			local,
			remote,
		}
	}

	#[tracing::instrument("connection", skip_all, err, fields(id = self.id))]
	pub async fn run(self) -> anyhow::Result<()> {
		let session = moq_transfork::Session::accept(self.session).await?;

		tokio::select! {
			res = Self::run_consumer(session.clone(), self.local.subscribe(), self.remote) => res,
			res = Self::run_producer(session, self.local) => res,
		}
	}

	async fn run_consumer(
		mut session: Session,
		mut local: AnnouncedConsumer,
		mut remote: AnnouncedConsumer,
	) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				Some(broadcast) = local.next() => session.publish(broadcast)?,
				Some(broadcast) = remote.next() => session.publish(broadcast)?,
				else => return Ok(())
			}
		}
	}

	async fn run_producer(session: Session, local: AnnouncedProducer) -> anyhow::Result<()> {
		let mut announced = session.announced();

		while let Some(broadcast) = announced.next().await {
			let active = local.insert(broadcast.clone())?;

			tokio::spawn(
				async move {
					broadcast.closed().await.ok();
					drop(active);
				}
				.in_current_span(),
			);
		}

		Ok(())
	}
}
