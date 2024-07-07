use super::{OrClose, SessionError, Stream};
use crate::{message, BroadcastReader};

pub struct Announce {
	broadcast: BroadcastReader,
	stream: Stream,
}

impl Announce {
	#[tracing::instrument("announce", skip_all, err, fields(broadcast=broadcast.name))]
	pub async fn open(session: &mut web_transport::Session, broadcast: BroadcastReader) -> Result<Self, SessionError> {
		let stream = Stream::open(session, message::Stream::Announce).await?;

		let mut this = Self { broadcast, stream };
		this.open_inner().await.or_close(&mut this.stream)?;

		Ok(this)
	}

	async fn open_inner(&mut self) -> Result<(), SessionError> {
		let announce = message::Announce {
			broadcast: self.broadcast.name.clone(),
		};

		self.stream.writer.encode(&announce).await?;

		let _ok = self.stream.reader.decode::<message::AnnounceOk>().await?;
		tracing::info!("ok");

		Ok(())
	}

	#[tracing::instrument("announce", skip_all, err, fields(broadcast=self.broadcast.name))]
	pub async fn run(mut self) -> Result<(), SessionError> {
		tokio::select! {
			res = self.stream.reader.closed() => res.map_err(SessionError::from),
			res = self.broadcast.closed() => res.map_err(SessionError::from),
		}
		.or_close(&mut self.stream)
	}

	pub fn id(&self) -> &str {
		&self.broadcast.name
	}
}
