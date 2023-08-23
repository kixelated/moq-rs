use crate::Object;

use anyhow::Context;

use tokio::task::JoinSet;

use webtransport_generic::Session;

pub struct Receiver<S: Session> {
	session: S,

	// Streams that we've accepted but haven't read the header from yet.
	streams: JoinSet<anyhow::Result<(Object, S::RecvStream)>>,
}

impl<S: Session> Receiver<S> {
	pub fn new(session: S) -> Self {
		Self {
			session,
			streams: JoinSet::new(),
		}
	}

	pub async fn recv(&mut self) -> anyhow::Result<(Object, S::RecvStream)> {
		loop {
			tokio::select! {
				res = self.session.accept_uni() => {
					let stream = res.context("failed to accept stream")?;
					self.streams.spawn(async move { Self::read(stream).await });
				},
				res = self.streams.join_next(), if !self.streams.is_empty() => {
					return res.unwrap().context("failed to run join set")?;
				}
			}
		}
	}

	async fn read(mut stream: S::RecvStream) -> anyhow::Result<(Object, S::RecvStream)> {
		let header = Object::decode(&mut stream).await?;
		Ok((header, stream))
	}
}
