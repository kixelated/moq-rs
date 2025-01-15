use url::Url;
use wasm_bindgen::prelude::*;

use web_sys::HtmlCanvasElement;

use super::{Backend, Controls, ControlsSend, Status, StatusRecv};
use crate::{Error, Result};

#[wasm_bindgen]
pub struct Watch {
	controls: ControlsSend,
	status: StatusRecv,
}

#[wasm_bindgen]
impl Watch {
	#[wasm_bindgen(constructor)]
	pub fn new() -> Self {
		let controls = Controls::default().baton();
		let status = Status::default().baton();

		let backend = Backend::new(controls.1, status.0);
		backend.start();

		Self {
			controls: controls.0,
			status: status.1,
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
	pub fn paused(&self) -> bool {
		self.controls.paused.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_paused(&mut self, paused: bool) {
		self.controls.paused.set(paused);
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

	#[wasm_bindgen(getter)]
	pub fn canvas(&self) -> Option<HtmlCanvasElement> {
		self.controls.canvas.get()
	}

	#[wasm_bindgen(setter)]
	pub fn set_canvas(&mut self, canvas: Option<HtmlCanvasElement>) {
		self.controls.canvas.set(canvas);
	}

	#[wasm_bindgen(getter)]
	pub fn error(&self) -> Option<String> {
		self.status.error.get().map(|e| e.to_string())
	}

	pub async fn on_state(&self, callback: js_sys::Function) -> Result<()> {
		let mut state = self.status.state.clone();
		while let Some(state) = state.next().await {
			callback.call1(&JsValue::NULL, &JsValue::from(state)).unwrap();
		}

		Ok(())
	}
}
