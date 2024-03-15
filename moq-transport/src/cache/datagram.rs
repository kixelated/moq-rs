/// Static information about the datagram.
#[derive(Clone, Debug)]
pub struct Info {
	pub object_id: u64,
	pub group_id: u64,
	pub send_order: u64,
	pub payload: bytes::Bytes,
}
