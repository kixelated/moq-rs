use std::collections::{hash_map::Entry, HashMap};

use baton::Baton;
use moq_karp::moq_transfork::{Announced, AnnouncedConsumer, AnnouncedMatch, AnnouncedProducer};
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

	room: Option<String>,
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

			room: None,
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

						self.room = Some(url.path().to_string());
						self.connect = Some(Connect::new(addr));
					} else {
						self.room = None;
						self.connect = None;
					}
				},
				Some(session) = async { Some(self.connect.as_mut()?.established().await) } => {
					let session = session?;
					self.producer.reset();
					let room = self.room.as_ref().unwrap();
					tracing::info!(?room, "connected to remote");
					let filter = format!("{}/*/.catalog", room);
					self.announced = Some(session.announced(filter));
					self.connect = None;
				},
				Some(announce) = async { Some(self.announced.as_mut()?.next().await) } => {
					let announce = announce.ok_or(Error::Closed)?;
					match announce {
						Announced::Active(am) => self.announced(am),
						Announced::Ended(am) => self.unannounced(am),
						Announced::Live => { self.producer.live(); },
					}
				},
				else => return Ok(()),
			}
		}
	}

	// Parse the user's name out of the "name/id" pair
	fn parse_name(am: AnnouncedMatch) -> std::result::Result<String, AnnouncedMatch> {
		match am.capture().find("/") {
			Some(index) => {
				// Make sure there's only one slash for bonus points
				if am.capture()[index + 1..].contains("/") {
					return Err(am);
				}

				let mut capture = am.to_capture();
				capture.truncate(index);
				Ok(capture)
			}
			None => Err(am),
		}
	}

	fn announced(&mut self, am: AnnouncedMatch) {
		let name = match Self::parse_name(am) {
			Ok(name) => name,
			Err(name) => {
				tracing::warn!(?name, "failed to parse track name");
				return;
			}
		};

		// Deduplicate based on the name so we don't announce the same person twice.
		match self.unique.entry(name.clone()) {
			Entry::Occupied(mut entry) => {
				*entry.get_mut() += 1;
			}
			Entry::Vacant(entry) => {
				entry.insert(1);
				self.producer.announce(name);
			}
		}
	}

	fn unannounced(&mut self, am: AnnouncedMatch) {
		let name = match Self::parse_name(am) {
			Ok(name) => name,
			Err(_) => return,
		};

		// Deduplicate based on the name so we don't unannounce the same person twice.
		if let Entry::Occupied(mut entry) = self.unique.entry(name.clone()) {
			*entry.get_mut() -= 1;

			if *entry.get() == 0 {
				entry.remove();
				self.producer.unannounce(&name);
			}
		}
	}
}
