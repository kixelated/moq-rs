use super::watch;
use std::sync::Arc;

// Use Arc to avoid cloning the data for each subscriber.
pub type Shared = Arc<Vec<u8>>;

// TODO combine fragments into the same buffer, instead of separate buffers.

pub type Publisher = watch::Publisher<Shared>;
pub type Subscriber = watch::Subscriber<Shared>;
