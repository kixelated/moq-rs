use super::Version;
use crate::coding::{Decode, Encode, Params, Size};

// Sent by the server in response to a client.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
pub struct Server {
	// The list of supported versions in preferred order.
	selected: Version,

	// A generic list of paramters.
	params: Params,
}

impl Decode for Server {
	fn decode<B: bytes::Buf>(r: &mut B) -> anyhow::Result<Self> {
		let selected = Version::decode(r)?;
		let params = Params::decode(r)?;

		Ok(Self { selected, params })
	}
}

impl Encode for Server {
	fn encode<B: bytes::BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.selected.encode(w)?;
		self.params.encode(w)?;

		Ok(())
	}
}

impl Size for Server {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.selected.size()? + self.params.size()?)
	}
}
