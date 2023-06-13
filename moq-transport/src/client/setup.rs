use super::Version;
use crate::coding::{Decode, Encode, Params, Size};

// Sent by the client to setup up the session.
pub struct Setup {
	// NOTE: This is not a message type, but rather the control stream header.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/138

	// The list of supported versions in preferred order.
	supported: Vec<Version>,

	// A generic list of paramters.
	params: Params,
}

impl Decode for Setup {
	fn decode<B: bytes::Buf>(r: &mut B) -> anyhow::Result<Self> {
		let supported = Vec::decode(r)?;
		let params = Params::decode(r)?;

		Ok(Self { supported, params })
	}
}

impl Encode for Setup {
	fn encode<B: bytes::BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.supported.encode(w)?;
		self.params.encode(w)?;

		Ok(())
	}
}

impl Size for SetupClient {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.supported.size()? + self.params.size()?)
	}
}
