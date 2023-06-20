use super::{update, Broadcast};

pub type Publisher = update::map::Publisher<String, Broadcast>;
pub type Subscriber = update::map::Subscriber<String, Broadcast>;
pub type Delta = update::map::Delta<String, Broadcast>;

#[derive(Clone)]
pub struct Broadcasts {
	pub publish: Publisher,
	pub subscribe: Subscriber,
}

impl Broadcasts {
	pub fn new() -> Self {
		Self::default()
	}
}

impl Default for Broadcasts {
	fn default() -> Self {
		let state = update::Shared::default();

		Self {
			publish: Publisher::new(state.clone()),
			subscribe: Subscriber::new(state),
		}
	}
}
