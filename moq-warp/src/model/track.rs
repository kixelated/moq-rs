use super::{segment, watch};

pub struct Publisher {
	pub name: String,

	pub segments: watch::Publisher<segment::Subscriber>,
}

impl Publisher {
	pub fn new(name: String) -> Publisher {
		Self {
			name,
			segments: watch::Publisher::new(),
		}
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber {
			name: self.name.clone(),
			segments: self.segments.subscribe(),
		}
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub name: String,

	// A list of segments, which are independently decodable.
	pub segments: watch::Subscriber<segment::Subscriber>,
}
