mod connection;
mod error;
mod meet;
mod publish;
mod watch;

pub use connection::*;
pub use error::*;
pub use meet::*;
pub use publish::*;
pub use watch::*;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
	// print pretty errors in wasm https://github.com/rustwasm/console_error_panic_hook
	// This is not needed for tracing_wasm to work, but it is a common tool for getting proper error line numbers for panics.
	console_error_panic_hook::set_once();

	let config = wasm_tracing::WASMLayerConfigBuilder::new()
		.set_max_level(tracing::Level::INFO)
		.build();
	wasm_tracing::set_as_global_default_with_config(config);
}
