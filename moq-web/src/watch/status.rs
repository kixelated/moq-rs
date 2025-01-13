use baton::Baton;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::Error;

#[derive(Debug, Default, Copy, Clone)]
#[wasm_bindgen]
pub enum State {
	#[default]
	Init,
	Connecting,
	Connected,
	Offline,
	Active,
	Error,
}

#[derive(Debug, Default, Baton)]
pub struct Status {
	pub state: State,
	pub error: Option<Error>,
}
