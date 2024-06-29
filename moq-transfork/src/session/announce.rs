use crate::{coding, message, BroadcastReader};

use super::SessionError;

pub struct Announce {
	broadcast: BroadcastReader,
	stream: coding::Stream,
}

impl Announce {
	pub fn new(stream: coding::Stream, broadcast: BroadcastReader) -> Self {
		Self { broadcast, stream }
	}

	#[tracing::instrument("start announce", skip(self), fields(stream = &self.stream.id(), broadcast=self.broadcast.name))]
	pub async fn start(&mut self) -> Result<(), SessionError> {
		let res = self.start_inner().await;

		if let Err(err) = &res {
			tracing::warn!(?err);
			self.stream.close(err.code());
		}

		Ok(())
	}

	async fn start_inner(&mut self) -> Result<(), SessionError> {
		let announce = message::Announce {
			broadcast: self.broadcast.name.clone(),
		};
		tracing::info!(?announce);
		self.stream.writer.encode(&announce).await?;

		let ok = self.stream.reader.decode::<message::AnnounceOk>().await?;
		tracing::info!(?ok);

		Ok(())
	}

	#[tracing::instrument("run announce", skip(self), fields(stream = &self.stream.id(), broadcast=self.broadcast.name))]
	pub async fn run(&mut self) {
		let res = tokio::select! {
			res = self.stream.reader.closed() => res.map_err(SessionError::from),
			res = self.broadcast.closed() => res.map_err(SessionError::from),
		};

		if let Err(err) = res {
			tracing::warn!(?err);
			self.stream.writer.reset(err.code());
		}
	}

	pub fn id(&self) -> &str {
		&self.broadcast.name
	}
}
