use super::{watch, Segment};
use std::fmt;

pub struct Publisher {
	pub name: String,

	pub segments: watch::Publisher<Segment>,
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Publisher").field("name", &self.name).finish()
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub name: String,

	// A list of segments, which are independently decodable.
	pub segments: watch::Subscriber<Segment>,
}

pub fn new(name: String) -> (Publisher, Subscriber) {
	let publisher = Publisher {
		name,
		segments: watch::Publisher::new(),
	};

	let subscriber = Subscriber {
		name,
		segments: publisher.segments.subscribe(),
	};

	(publisher, subscriber)
}
