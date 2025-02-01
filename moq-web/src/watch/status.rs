use baton::Baton;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{ConnectionStatus, Error, Result};

// Sent from the backend to the frontend.
#[derive(Debug, Default, Baton)]
pub(super) struct Status {
	pub connection: ConnectionStatus,
	pub render: RendererStatus,
	pub error: Option<Error>,
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[wasm_bindgen]
pub enum RendererStatus {
	// No video track has been configured.
	#[default]
	Idle,

	// Rendering has been paused.
	Paused,

	// No frame has been received for >1s
	Buffering,

	// Rendering is active.
	Live,
}

// Unfortunately, we need wrappers because `wasm_bindgen` doesn't support many types.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WatchStatus {
	status: StatusRecv,
}

#[wasm_bindgen]
impl WatchStatus {
	pub(super) fn new(status: StatusRecv) -> Self {
		Self { status }
	}

	pub async fn connection(&mut self) -> Result<ConnectionStatus> {
		match self.status.connection.next().await {
			Some(state) => Ok(state),
			None => Err(self.error().await),
		}
	}

	pub async fn renderer(&mut self) -> Result<RendererStatus> {
		match self.status.render.next().await {
			Some(state) => Ok(state),
			None => Err(self.error().await),
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
