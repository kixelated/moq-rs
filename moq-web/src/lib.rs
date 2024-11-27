mod decoder;
mod error;
//mod publish;
mod renderer;
mod run;
mod session;
mod watch;

pub use error::*;
//pub use publish::*;
pub use watch::*;

use decoder::*;
use renderer::*;
use run::*;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    // print pretty errors in wasm https://github.com/rustwasm/console_error_panic_hook
    // This is not needed for tracing_wasm to work, but it is a common tool for getting proper error line numbers for panics.
    console_error_panic_hook::set_once();

    let config = wasm_tracing::WASMLayerConfigBuilder::new()
        .set_max_level(tracing::Level::DEBUG)
        .build();
    wasm_tracing::set_as_global_default_with_config(config);
}
