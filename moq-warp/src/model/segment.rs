use super::watch;

use bytes::Bytes;
use moq_transport::VarInt;
use std::ops::Deref;
use std::sync::Arc;
use std::time;

#[derive(Clone, Debug)]
pub struct Info {
	// The sequence number of the segment within the track.
	pub sequence: VarInt,

	// The priority of the segment within the BROADCAST.
	pub send_order: VarInt,

	// The time at which the segment expires for cache purposes.
	pub expires: Option<time::Instant>,
}

pub struct Publisher {
	pub info: Arc<Info>,

	// A list of fragments that make up the segment.
	pub fragments: watch::Publisher<Bytes>,
}

impl Publisher {
	pub fn new(info: Info) -> Self {
		Self {
			info: Arc::new(info),
			fragments: watch::Publisher::new(),
		}
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber {
			info: self.info.clone(),
			fragments: self.fragments.subscribe(),
		}
	}
}

impl Deref for Publisher {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct Subscriber {
	pub info: Arc<Info>,

	// A list of fragments that make up the segment.
	pub fragments: watch::Subscriber<Bytes>,
}

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
