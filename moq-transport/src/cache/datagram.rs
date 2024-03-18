use crate::{publisher, ServeError};

/// Static information about the datagram.
#[derive(Clone, Debug)]
pub struct Datagram {
	pub object_id: u64,
	pub group_id: u64,
	pub send_order: u64,
	pub payload: bytes::Bytes,
}

impl Datagram {
	pub fn serve(self, mut dst: publisher::Subscribe) -> Result<(), ServeError> {
		dst.serve_datagram(publisher::Datagram {
			group_id: self.group_id,
			object_id: self.object_id,
			send_order: self.send_order,
			payload: self.payload,
		})?;

		Ok(())
	}
}
