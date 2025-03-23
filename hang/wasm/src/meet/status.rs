use baton::Baton;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{ConnectionStatus, Error, Result};

// Sent from the backend to the frontend.
#[derive(Debug, Default, Baton)]
pub(super) struct Status {
	pub connection: ConnectionStatus,
	pub error: Option<Error>,
}

// Unfortunately, we need wrappers because `wasm_bindgen` doesn't support many types.
#[wasm_bindgen]
#[derive(Clone)]
pub struct MeetStatus {
	status: StatusRecv,
}

#[wasm_bindgen]
impl MeetStatus {
	pub(super) fn new(status: StatusRecv) -> Self {
		Self { status }
	}

	pub async fn connection(&mut self) -> Result<ConnectionStatus> {
		match self.status.connection.next().await {
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
