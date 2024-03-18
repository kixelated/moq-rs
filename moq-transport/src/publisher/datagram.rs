pub struct Datagram {
	pub group_id: u64,
	pub object_id: u64,
	pub send_order: u64,
	pub payload: bytes::Bytes,
}
