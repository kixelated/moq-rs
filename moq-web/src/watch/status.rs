use baton::Baton;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Error;

#[derive(Debug, Default, Copy, Clone)]
#[wasm_bindgen]
pub enum WatchState {
	#[default]
	Init,
	Connecting,
	Connected,
	Playing,
	Offline,
	Closed,
}

#[derive(Debug, Default, Baton)]
pub struct Status {
	pub state: WatchState,
	pub error: Option<Error>,
}
