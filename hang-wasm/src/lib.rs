mod command;
mod connection;
mod error;
mod publish;
mod room;
mod watch;

pub use command::*;
pub use connection::*;
pub use error::*;
pub use publish::*;
pub use room::*;
pub use watch::*;

use wasm_bindgen::prelude::*;

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

	// Get the worker Worker scope
	let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();

	let closure = Closure::wrap(Box::new(move |event: web_sys::MessageEvent| {
		handle_message(event.data());
	}) as Box<dyn FnMut(_)>);

	global.set_onmessage(Some(closure.as_ref().unchecked_ref()));
	closure.forget(); // Leak it — persistent handler
}

fn handle_message(data: JsValue) {
	let command = Command::from_message(data).unwrap();
	println!("Received message: {:?}", command);
}

fn post_message(data: JsValue) {
	let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();
	global.post_message(&data);
}
