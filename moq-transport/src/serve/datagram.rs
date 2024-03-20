use std::fmt;

/// Static information about the datagram.
#[derive(Clone)]
pub struct Datagram {
	pub object_id: u64,
	pub group_id: u64,
	pub send_order: u64,
	pub payload: bytes::Bytes,
}

impl fmt::Debug for Datagram {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Datagram")
			.field("object_id", &self.object_id)
			.field("group_id", &self.group_id)
			.field("send_order", &self.send_order)
			.field("payload", &self.payload.len())
			.finish()
	}
}
