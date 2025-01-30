use std::time::Duration;

use baton::Baton;
use url::Url;
use wasm_bindgen::prelude::*;

use web_sys::OffscreenCanvas;

use super::{Backend, BackendState, RenderState};
use crate::{Error, Result};

// Sent from the frontend to the backend.
#[derive(Debug, Baton)]
pub(super) struct Controls {
	pub url: Option<Url>,
	pub paused: bool,
	pub volume: f64,
	pub canvas: Option<web_sys::OffscreenCanvas>,

	// Play media faster until this latency is reached.
	pub latency: Duration,
}

impl Default for Controls {
	fn default() -> Self {
		Self {
			url: None,
			paused: false,
			volume: 1.0,
			canvas: None,
			latency: Duration::ZERO,
		}
	}
}

// Sent from the backend to the frontend.
#[derive(Debug, Default, Baton)]
pub(super) struct Status {
	pub backend: BackendState,
	pub render: RenderState,
	pub error: Option<Error>,
}

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

	pub fn status(&self) -> WatchStatus {
		WatchStatus {
			status: self.status.clone(),
		}
	}
}

impl Default for Watch {
	fn default() -> Self {
		Self::new()
	}
}

// Unfortunately, we need wrappers because `wasm_bindgen` doesn't support many types.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WatchStatus {
	status: StatusRecv,
}

#[wasm_bindgen]
impl WatchStatus {
	pub async fn backend_state(&mut self) -> Result<BackendState> {
		match self.status.backend.next().await {
			None => Err(self.error().await),
			Some(state) => Ok(state),
		}
	}

	pub async fn render_state(&mut self) -> Result<RenderState> {
		match self.status.render.next().await {
			None => Err(self.error().await),
			Some(state) => Ok(state),
		}
	}

	async fn error(&mut self) -> Error {
		if let Some(err) = self.status.error.get() {
			return err.clone();
		}

		self.status
			.error
			.next()
			.await
			.as_ref()
			.expect("status closed without error")
			.as_ref()
			.expect("error was set to None")
			.clone()
	}
}
