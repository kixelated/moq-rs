use std::time::Duration;

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

	pub fn set_url(&mut self, url: Option<String>) -> Result<()> {
		let url = match url {
			Some(url) => Url::parse(&url).map_err(|_| Error::InvalidUrl(url.to_string()))?.into(),
			None => None,
		};
		self.controls.url.set(url);
		Ok(())
	}

	pub fn set_paused(&mut self, paused: bool) {
		self.controls.paused.set(paused);
	}

	pub fn set_volume(&mut self, volume: f64) {
		self.controls.volume.set(volume);
	}

	pub fn set_canvas(&mut self, canvas: Option<OffscreenCanvas>) {
		self.controls.canvas.set(canvas);
	}

	pub fn set_latency(&mut self, latency: u32) {
		self.controls.latency.set(Duration::from_millis(latency as _));
	}

	pub fn set_latency_max(&mut self, latency_max: u32) {
		self.controls.latency_max.set(Duration::from_millis(latency_max as _));
	}

	pub fn error(&self) -> Option<String> {
		self.status.error.get().map(|e| e.to_string())
	}

	pub fn states(&self) -> WatchStates {
		WatchStates {
			state: self.status.state.clone(),
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
	Live,
	Offline,
	Error,
}

// Unfortunately, we need this wrapper because `wasm_bindgen` doesn't support generics.
#[wasm_bindgen]
pub struct WatchStates {
	state: baton::Recv<WatchState>,
}

#[wasm_bindgen]
impl WatchStates {
	pub async fn next(&mut self) -> Option<WatchState> {
		self.state.next().await
	}
}
