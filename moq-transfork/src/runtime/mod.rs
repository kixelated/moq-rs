mod queue;
mod watch;

pub use queue::*;
pub use watch::*;

// It's not pretty, but we have per-platform implementations.
// This is because Send is pretty fundamentally broken in Rust.
// WASM doesn't support threads, so we use a simpler set of primitives.
// One day we'll support a "send" feature but for now this is fine.

// Arc and Mutex based
#[cfg(any(not(target_arch = "wasm32"), target_os = "wasi"))]
#[path = "tokio.rs"]
mod platform;

// Rc and RefCell based
#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
#[path = "wasm.rs"]
mod platform;

pub use platform::*;
