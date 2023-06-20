mod source;
pub use source::Source;

// TODO move each to its own file
mod model;
pub use model::*;

mod watch;
use watch::{Producer, Subscriber};

pub mod broadcasts;
pub use broadcasts::Broadcasts;

mod update;
