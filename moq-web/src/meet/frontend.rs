use moq_karp::moq_transfork::{Announced, AnnouncedConsumer, AnnouncedProducer};
use url::Url;
use wasm_bindgen::prelude::*;

use super::{Backend, Controls, ControlsSend, Status, StatusRecv};
use crate::{Error, MeetStatus, Result};

#[wasm_bindgen]
pub struct Meet {
	controls: ControlsSend,
	status: StatusRecv,
	announced: AnnouncedConsumer,
}

#[wasm_bindgen]
impl Meet {
	#[wasm_bindgen(constructor)]
	pub fn new() -> Self {
		let producer = AnnouncedProducer::new();
		let consumer = producer.subscribe();

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

	pub fn announced(&self) -> MeetAnnounced {
		MeetAnnounced::new(self.announced.clone())
	}

	pub fn status(&self) -> MeetStatus {
		MeetStatus::new(self.status.clone())
	}
}

impl Default for Meet {
	fn default() -> Self {
		Self::new()
	}
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum MeetAction {
	Join,
	Leave,
	Live,
}

#[wasm_bindgen(getter_with_clone)]
pub struct MeetAnnounce {
	pub action: MeetAction,
	pub name: String,
}

#[wasm_bindgen]
pub struct MeetAnnounced {
	inner: AnnouncedConsumer,
}

#[wasm_bindgen]
impl MeetAnnounced {
	fn new(inner: AnnouncedConsumer) -> Self {
		Self { inner }
	}

	pub async fn next(&mut self) -> Option<MeetAnnounce> {
		let next = self.inner.next().await?;
		tracing::info!(?next);
		Some(match next {
			Announced::Active(suffix) => MeetAnnounce {
				action: MeetAction::Join,
				name: suffix[0].clone(),
			},
			Announced::Ended(suffix) => MeetAnnounce {
				action: MeetAction::Leave,
				name: suffix[0].clone(),
			},
			Announced::Live => MeetAnnounce {
				action: MeetAction::Live,
				name: "".to_string(),
			},
		})
	}
}
