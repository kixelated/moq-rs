use url::Url;
use wasm_bindgen::prelude::*;

use web_sys::OffscreenCanvas;

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

	pub fn url(&mut self, url: Option<String>) -> Result<()> {
		let url = url.map(|u| Url::parse(&u)).transpose().map_err(|_| Error::InvalidUrl)?;
		self.controls.url.set(url);
		Ok(())
	}

	pub fn paused(&mut self, paused: bool) {
		self.controls.paused.set(paused);
	}

	pub fn volume(&mut self, volume: f64) {
		self.controls.volume.set(volume);
	}

	pub fn canvas(&mut self, canvas: Option<OffscreenCanvas>) {
		self.controls.canvas.set(canvas);
	}

	pub fn states(&self) -> WatchStates {
		WatchStates {
			status: self.status.state.clone(),
		}
	}
}

impl Default for Watch {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Debug, Default, Copy, Clone)]
#[wasm_bindgen]
pub enum WatchState {
	#[default]
	Idle,
	Connecting,
	Connected,
	Playing,
	Offline,
	Error,
}

// Unfortunately, we need this wrapper because `wasm_bindgen` doesn't support generics.
#[wasm_bindgen]
pub struct WatchStates {
	status: baton::Recv<WatchState>,
}

#[wasm_bindgen]
impl WatchStates {
	pub async fn next(&mut self) -> Option<WatchState> {
		self.status.next().await
	}
}
