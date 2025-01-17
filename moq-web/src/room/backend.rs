use baton::Baton;
use moq_karp::moq_transfork::{Announced, AnnouncedConsumer, AnnouncedProducer, Path};
use url::Url;
use wasm_bindgen_futures::spawn_local;

use crate::{Connect, Error, Result};

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub url: Option<Url>,
}

pub struct Backend {
	controls: ControlsRecv,

	room: Path,
	connect: Option<Connect>,
	announced: Option<AnnouncedConsumer>,

	producer: AnnouncedProducer,
}

impl Backend {
	pub fn new(controls: ControlsRecv, producer: AnnouncedProducer) -> Self {
		Self {
			controls,
			producer,

			room: Path::default(),
			connect: None,
			announced: None,
		}
	}

	pub fn start(mut self) {
		spawn_local(async move {
			if let Err(err) = self.run().await {
				tracing::error!(?err, "backend error");
			}
		});
	}

	async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				url = self.controls.url.next() => {
					let url = url.ok_or(Error::Closed)?;

					// TODO unannounce existing entries?
					self.announced = None;

					if let Some(url) = url{
						// Connect using the base of the URL.
						let mut addr = url.clone();
						addr.set_fragment(None);
						addr.set_query(None);
						addr.set_path("");

						self.room = url.path_segments().ok_or(Error::InvalidUrl(url.to_string()))?.collect();
						self.connect = Some(Connect::new(addr));
					} else {
						self.room = Path::default();
						self.connect = None;
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let session = session?;
					self.producer.reset();
					tracing::info!(?self.room, "connected to remote");
					self.announced = Some(session.announced(self.room.clone()));
					self.connect = None;
				},
				Some(announce) = async { Some(self.announced.as_mut()?.next().await) } => {
					let announce = announce.ok_or(Error::Closed)?;
					match announce {
						Announced::Active(suffix) => self.announced(suffix),
						Announced::Ended(suffix) => self.unannounced(suffix),
						Announced::Live => { self.producer.live(); },
					}
				},
				else => return Ok(()),
			}
		}
	}

	fn announced(&mut self, suffix: Path) {
		if suffix.len() != 2 {
			return;
		}

		self.producer.announce(suffix);
	}

	fn unannounced(&mut self, suffix: Path) {
		if suffix.len() != 2 {
			return;
		}

		self.producer.unannounce(&suffix);
	}
}
