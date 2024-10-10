use super::Result;
use tokio::io::AsyncWrite;

use crate::{catalog, media};

// Converts Karp -> fMP4
pub struct Export<W: AsyncWrite + Unpin> {
	output: W,
	broadcast: media::BroadcastConsumer,
}

// TODO
impl<W: AsyncWrite + Unpin> Export<W> {
	pub async fn init(input: moq_transfork::BroadcastConsumer, output: W) -> Result<Self> {
		let broadcast = media::BroadcastConsumer::load(input).await?;
		Ok(Self { broadcast, output })
	}

	pub async fn run(self) -> Result<()> {
		todo!();
		Ok(())
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		self.broadcast.catalog()
	}
}
