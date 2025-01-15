use url::Url;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlVideoElement, MediaStream};

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
	pub fn new() -> Self {
		let controls = Controls::default().baton();
		let status = Status::default().baton();

		let backend = Backend::new(controls.1, status.0);
		backend.start();

		Self {
			controls: controls.0,
			_status: status.1,
		}
	}

	#[wasm_bindgen(getter)]
	pub fn url(&self) -> Option<String> {
		self.controls.url.get().map(|u| u.to_string())
	}

	#[wasm_bindgen(setter)]
	pub fn set_url(&mut self, url: Option<String>) -> Result<()> {
		let url = url.map(|u| Url::parse(&u)).transpose().map_err(|_| Error::InvalidUrl)?;
		self.controls.url.set(url);
		Ok(())
	}

	#[wasm_bindgen(getter)]
	pub fn preview(&self) -> Option<HtmlVideoElement> {
		self.controls.preview.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_preview(&mut self, preview: Option<HtmlVideoElement>) {
		self.controls.preview.set(preview)
	}

	#[wasm_bindgen(getter)]
	pub fn media(&self) -> Option<MediaStream> {
		self.controls.media.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_media(&mut self, media: Option<MediaStream>) {
		self.controls.media.set(media)
	}

	#[wasm_bindgen(getter)]
	pub fn volume(&self) -> f64 {
		self.controls.volume.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_volume(&mut self, volume: f64) {
		self.controls.volume.set(volume);
	}

	#[wasm_bindgen(getter)]
	pub fn closed(&self) -> bool {
		self.controls.close.get()
	}

	pub fn close(&mut self) {
		self.controls.close.set(true);
	}
}
