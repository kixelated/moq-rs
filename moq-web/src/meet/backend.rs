use std::collections::{hash_map::Entry, HashMap};

use baton::Baton;
use moq_karp::moq_transfork::{self, Announced, AnnouncedConsumer, AnnouncedMatch, AnnouncedProducer};
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

					// TODO make a helper in karp for this
					let filter = moq_transfork::Filter::Wildcard {
						prefix: format!("{}/", path),
						suffix: "/catalog.json".to_string(),
					};

					self.announced = Some(session.announced(filter));
					self.status.connection.update(ConnectionStatus::Connected);
				},
				Some(announce) = async { Some(self.announced.as_mut()?.next().await) } => {
					let announce = announce.ok_or(Error::Closed)?;
					match announce {
						Announced::Active(track) => self.announced(track),
						Announced::Ended(track) => self.unannounced(track),
						Announced::Live => self.live(),
					}
				},
				else => return Ok(()),
			}
		}
	}

	// Parse the user's name out of the "name/id" pair
	fn parse_name(track: AnnouncedMatch) -> std::result::Result<String, AnnouncedMatch> {
		match track.capture().find("/") {
			Some(index) => {
				// Make sure there's only one slash for bonus points
				if track.capture()[index + 1..].contains("/") {
					return Err(track);
				}

				let mut capture = track.to_capture();
				capture.truncate(index);
				Ok(capture)
			}
			None => Err(track),
		}
	}

	fn announced(&mut self, track: AnnouncedMatch) {
		let name = match Self::parse_name(track) {
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

		self.update_status();
	}

	fn unannounced(&mut self, track: AnnouncedMatch) {
		let name = match Self::parse_name(track) {
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
