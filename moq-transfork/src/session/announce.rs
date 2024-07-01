use std::fmt;

use crate::{
	coding::{self, Stream},
	message, BroadcastReader,
};

use super::SessionError;

pub struct Announce {
	broadcast: BroadcastReader,
	stream: coding::Stream,
}

impl Announce {
	pub async fn open(session: &mut web_transport::Session, broadcast: BroadcastReader) -> Result<Self, SessionError> {
		let stream = Stream::open(session, message::Control::Announce).await?;
		let mut this = Self { broadcast, stream };

		if let Err(err) = this.open_inner().await {
			this.stream.close(err.code());
			return Err(err);
		}

		Ok(this)
	}

	#[tracing::instrument("announce", skip_all, err, fields(broadcast=self.broadcast.name, stream=self.stream.id))]
	async fn open_inner(&mut self) -> Result<(), SessionError> {
		let announce = message::Announce {
			broadcast: self.broadcast.name.clone(),
		};

		tracing::info!(?announce);
		self.stream.writer.encode(&announce).await?;

		let ok = self.stream.reader.decode::<message::AnnounceOk>().await?;
		tracing::info!(?ok);

		Ok(())
	}

	#[tracing::instrument("announce", skip_all, err, fields(broadcast=self.broadcast.name, stream=self.stream.id))]
	pub async fn run(mut self) -> Result<(), SessionError> {
		let res = tokio::select! {
			res = self.stream.reader.closed() => res.map_err(SessionError::from),
			res = self.broadcast.closed() => res.map_err(SessionError::from),
		};

		if let Err(err) = &res {
			self.stream.close(err.code());
		}

		res
	}

	pub fn id(&self) -> &str {
		&self.broadcast.name
	}
}
