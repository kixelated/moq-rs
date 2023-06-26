mod file;
pub use file::*;

use crate::model::track;

use std::collections::HashMap;

// TODO move to model::Broadcast?
pub trait Source {
	fn subscribe(&self, name: &str) -> Option<track::Subscriber>;
}

#[derive(Clone, Default)]
pub struct MapSource(pub HashMap<String, track::Subscriber>);

impl Source for MapSource {
	fn subscribe(&self, name: &str) -> Option<track::Subscriber> {
		self.0.get(name).cloned()
	}
}
