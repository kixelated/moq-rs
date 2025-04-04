use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
#[wasm_bindgen]
pub enum ConnectionStatus {
	#[default]
	Disconnected,
	Connecting,
	Connected,

	// TODO move these two elsewhere or add support to Publish
	Offline,
	Live,
}
