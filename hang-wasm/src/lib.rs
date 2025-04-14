mod connect;
mod error;
mod publish;
mod rpc;
mod watch;

pub use connect::*;
pub use error::*;
pub use publish::*;
pub use rpc::*;
pub use watch::*;

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn start() {
	// print pretty errors in wasm https://github.com/rustwasm/console_error_panic_hook
	// This is not needed for tracing_wasm to work, but it is a common tool for getting proper error line numbers for panics.
	console_error_panic_hook::set_once();

	let config = wasm_tracing::WasmLayerConfig {
		max_level: tracing::Level::INFO,
		..Default::default()
	};
	wasm_tracing::set_as_global_default_with_config(config).expect("failed to install logger");

	wasm_bindgen_futures::spawn_local(run());
}

async fn run() {
	let mut backend = Backend::new();
	backend.run().await.unwrap();
}
