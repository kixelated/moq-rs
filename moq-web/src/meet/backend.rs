use std::collections::{hash_map::Entry, HashMap};

use baton::Baton;
use moq_karp::moq_transfork::{Announced, AnnouncedConsumer, AnnouncedProducer, Path};
use url::Url;
use wasm_bindgen_futures::spawn_local;

use crate::{Connect, ConnectionStatus, Error, Result};

use super::StatusSend;

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub url: Option<Url>,
}

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	connect: Option<Connect>,
	announced: Option<AnnouncedConsumer>,

	producer: AnnouncedProducer,
	unique: HashMap<String, usize>,
}

impl Backend {
	pub fn new(controls: ControlsRecv, status: StatusSend, producer: AnnouncedProducer) -> Self {
		Self {
			controls,
			status,
			producer,

			connect: None,
			announced: None,
			unique: HashMap::new(),
		}
	}

	pub fn start(mut self) {
		spawn_local(async move {
			if let Err(err) = self.run().await {
				self.status.error.set(Some(err));
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

					if let Some(url) = url {
						self.connect = Some(Connect::new(url));
						self.status.connection.update(ConnectionStatus::Connecting);
					} else {
						self.connect = None;
						self.status.connection.update(ConnectionStatus::Disconnected);
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					tracing::info!("connected to server");
					let session = session?;
					self.producer.reset();
					let path = self.connect.take().unwrap().path;
					self.announced = Some(session.announced(path));
					self.status.connection.update(ConnectionStatus::Connected);
				},
				Some(announce) = async { Some(self.announced.as_mut()?.next().await) } => {
					tracing::info!(?announce, "iannounce");
					let announce = announce.ok_or(Error::Closed)?;
					match announce {
						Announced::Active(suffix) => self.announced(suffix),
						Announced::Ended(suffix) => self.unannounced(suffix),
						Announced::Live => self.live(),
					}
				},
				else => return Ok(()),
			}
		}
	}

	fn announced(&mut self, suffix: Path) {
		// TODO only announce a single level deep
		if suffix.len() != 2 {
			return;
		}

		// Annoying that we have to do this.
		let name = suffix.first().cloned().unwrap();
		match self.unique.entry(name.clone()) {
			Entry::Occupied(mut entry) => {
				*entry.get_mut() += 1;
			}
			Entry::Vacant(entry) => {
				entry.insert(1);
				self.producer.announce(Path::new().push(name));
			}
		}

		self.update_status();
	}

	fn unannounced(&mut self, suffix: Path) {
		// TODO only announce a single level deep
		if suffix.len() != 2 {
			return;
		}

		// Annoying that we have to do this.
		let name = suffix.first().unwrap();
		if let Entry::Occupied(mut entry) = self.unique.entry(name.clone()) {
			*entry.get_mut() -= 1;

			if *entry.get() == 0 {
				entry.remove();
				self.producer.unannounce(&Path::new().push(name));
			}
		}

		self.update_status();
	}

	fn live(&mut self) {
		self.producer.live();
		self.update_status();
	}

	fn update_status(&mut self) {
		if self.producer.is_empty() {
			self.status.connection.update(ConnectionStatus::Offline);
		} else {
			self.status.connection.update(ConnectionStatus::Live);
		}
	}
}
