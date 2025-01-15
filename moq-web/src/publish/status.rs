use crate::Error;
use baton::Baton;
use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Debug, Default, Copy, Clone)]
#[wasm_bindgen]
pub enum PublishState {
	#[default]
	Init,
	Connecting,
	Connected,
	Closed,
}

#[derive(Debug, Default, Clone, Baton)]
pub struct Status {
	pub state: PublishState,
	pub error: Option<Error>,
}
