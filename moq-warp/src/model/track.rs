use super::{segment, watch};
use std::{error, fmt, time};

use moq_transport::VarInt;

pub struct Publisher {
	pub name: String,

	segments: watch::Publisher<Result<segment::Subscriber, Error>>,
}

impl Publisher {
	pub fn new(name: &str) -> Publisher {
		Self {
			name: name.to_string(),
			segments: watch::Publisher::new(),
		}
	}

	pub fn push_segment(&mut self, segment: segment::Subscriber) {
		self.segments.push(Ok(segment))
	}

	pub fn drain_segments(&mut self, before: time::Instant) {
		self.segments.drain(|segment| {
			if let Ok(segment) = segment {
				if let Some(expires) = segment.expires {
					return expires < before;
				}
			}

			false
		})
	}

	pub fn close(mut self, err: Error) {
		self.segments.push(Err(err))
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber {
			name: self.name.clone(),
			segments: self.segments.subscribe(),
		}
	}
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "track publisher: {:?}", self.name)
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub name: String,

	// A list of segments, which are independently decodable.
	segments: watch::Subscriber<Result<segment::Subscriber, Error>>,
}

impl Subscriber {
	pub async fn next_segment(&mut self) -> Result<segment::Subscriber, Error> {
		let res = self.segments.next().await;
		match res {
			None => Err(Error {
				code: VarInt::from_u32(0),
				reason: String::from("closed"),
			}),
			Some(res) => res,
		}
	}
}

impl fmt::Debug for Subscriber {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "track subscriber: {:?}", self.name)
	}
}

#[derive(Clone)]
pub struct Error {
	pub code: VarInt,
	pub reason: String,
}

impl error::Error for Error {}

impl fmt::Debug for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if !self.reason.is_empty() {
			write!(f, "track error ({}): {}", self.code, self.reason)
		} else {
			write!(f, "track error ({})", self.code)
		}
	}
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if !self.reason.is_empty() {
			write!(f, "track error ({}): {}", self.code, self.reason)
		} else {
			write!(f, "track error ({})", self.code)
		}
	}
}
