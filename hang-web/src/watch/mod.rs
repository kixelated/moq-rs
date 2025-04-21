mod command;
mod renderer;
mod video;

pub use command::*;
pub use renderer::*;
use url::Url;
pub use video::*;

use crate::{Connect, Result};
use hang::moq_lite::Session;

#[derive(Default)]
pub struct Watch {
	connect: Option<Connect>,

	broadcast: Option<hang::BroadcastConsumer>,
	catalogs: Option<hang::CatalogConsumer>,

	canvas: Option<web_sys::OffscreenCanvas>,
	latency: u32,
}

impl Watch {
	pub fn recv(&mut self, command: WatchCommand) -> Result<()> {
		match command {
			WatchCommand::Connect(url) => {
				self.connect = None;

				if let Some(url) = url {
					let url = Url::parse(&url)?;
					self.connect = Some(Connect::new(url));
				}
			}
			WatchCommand::Canvas(canvas) => self.canvas = canvas,
			WatchCommand::Latency(latency) => self.latency = latency,
		};

		Ok(())
	}

	fn connected(&mut self, connect: Connect, session: Session) -> Result<()> {
		tracing::info!("connected to server");

		Ok(())
	}

	async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let connect = self.connect.take().unwrap();
					self.connected(connect, session?)?;
				}
				Some(catalogs) = async { Some(self.broadcast.as_mut()?.catalog().await) } => {
					self.catalogs = Some(catalogs?);
				}
				else => return Ok(()),
			}
		}
	}
}
