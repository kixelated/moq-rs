use crate::VarInt;

// The header for a MoQ stream, which is sent as the first bytes on a QUIC stream.
// This is quite simple for now, but will expand in the future.
#[derive(Clone, Debug)]
pub struct Object {
	pub sequence: VarInt,
	pub send_order: i32,
}
