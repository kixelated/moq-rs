use super::{track, watch};

pub struct Publisher {
	pub namespace: String,

	pub tracks: watch::Publisher<track::Subscriber>,
}

impl Publisher {
	pub fn new(namespace: &str) -> Self {
		Self {
			namespace: namespace.to_string(),
			tracks: watch::Publisher::new(),
		}
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber {
			namespace: self.namespace.clone(),
			tracks: self.tracks.subscribe(),
		}
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub namespace: String,

	pub tracks: watch::Subscriber<track::Subscriber>,
}
