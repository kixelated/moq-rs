use std::collections::{hash_map::Entry, HashMap};

use baton::Baton;
use moq_karp::moq_transfork::{Announced, AnnouncedConsumer, AnnouncedProducer, Path};
use url::Url;
use wasm_bindgen_futures::spawn_local;

use crate::{Connect, Error, Result};

#[derive(Debug, Default, Baton)]
pub struct Controls {
	pub url: Option<Url>,
}

#[derive(Debug, Default, Baton)]
pub struct Status {
	pub error: Option<Error>,
}

pub struct Backend {
	controls: ControlsRecv,
	status: StatusSend,

	room: Path,
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

			room: Path::default(),
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
	}
}
