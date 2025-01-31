use moq_karp::moq_transfork::{Announced, AnnouncedConsumer, AnnouncedProducer};
use url::Url;
use wasm_bindgen::prelude::*;

use super::{Backend, Controls, ControlsSend, Status, StatusRecv};
use crate::{Error, Result};

#[wasm_bindgen]
pub struct Room {
	controls: ControlsSend,
	status: StatusRecv,
	announced: AnnouncedConsumer,
}

#[wasm_bindgen]
impl Room {
	#[wasm_bindgen(constructor)]
	pub fn new() -> Self {
		let producer = AnnouncedProducer::new();
		let consumer = producer.subscribe("*");

		let controls = Controls::default().baton();
		let status = Status::default().baton();

		let backend = Backend::new(controls.1, status.0, producer);
		backend.start();

		Self {
			controls: controls.0,
			status: status.1,
			announced: consumer,
		}
	}

	#[wasm_bindgen(setter)]
	pub fn set_url(&mut self, url: Option<String>) -> Result<()> {
		let url = match url {
			Some(url) => Url::parse(&url).map_err(|_| Error::InvalidUrl(url.to_string()))?.into(),
			None => None,
		};
		self.controls.url.set(url);

		Ok(())
	}

	pub fn announced(&self) -> RoomAnnounced {
		RoomAnnounced::new(self.announced.clone())
	}

	#[wasm_bindgen(getter)]
	pub fn error(&self) -> Option<String> {
		self.status.error.get().as_ref().map(|e| e.to_string())
	}
}

impl Default for Room {
	fn default() -> Self {
		Self::new()
	}
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum RoomAction {
	Join,
	Leave,
	Live,
}

#[wasm_bindgen(getter_with_clone)]
pub struct RoomAnnounce {
	pub action: RoomAction,
	pub name: String,
}

#[wasm_bindgen]
pub struct RoomAnnounced {
	inner: AnnouncedConsumer,
}

#[wasm_bindgen]
impl RoomAnnounced {
	fn new(inner: AnnouncedConsumer) -> Self {
		Self { inner }
	}

	pub async fn next(&mut self) -> Option<RoomAnnounce> {
		Some(match self.inner.next().await? {
			Announced::Active(am) => RoomAnnounce {
				action: RoomAction::Join,
				name: am.to_full(),
			},
			Announced::Ended(am) => RoomAnnounce {
				action: RoomAction::Leave,
				name: am.to_full(),
			},
			Announced::Live => RoomAnnounce {
				action: RoomAction::Live,
				name: "".to_string(),
			},
		})
	}
}
