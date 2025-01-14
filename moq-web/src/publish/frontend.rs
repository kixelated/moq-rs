use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::MediaStream;

use super::{Backend, Controls, ControlsSend, Status, StatusRecv};
use crate::{Error, Result};

#[wasm_bindgen]
pub struct Publish {
	controls: ControlsSend,
	_status: StatusRecv,
}

#[wasm_bindgen]
impl Publish {
	#[wasm_bindgen(constructor)]
	pub fn new(src: &str) -> Result<Self> {
		let src = Url::parse(src).map_err(|_| Error::InvalidUrl)?;

		let controls = Controls::default().baton();
		let status = Status::default().baton();
		let mut backend = Backend::new(src, controls.1, status.0);

		spawn_local(async move {
			if let Err(err) = backend.run().await {
				tracing::error!(?err, "backend error");
			} else {
				tracing::warn!("backend closed");
			}
		});

		Ok(Self {
			controls: controls.0,
			_status: status.1,
		})
	}

	pub fn capture(&mut self, media: Option<MediaStream>) {
		self.controls.media.send(media).ok();
	}

	pub fn volume(&mut self, value: f64) {
		self.controls.volume.send(value).ok();
	}

	pub fn close(&mut self) {
		self.controls.close.send(true).ok();
	}
}
