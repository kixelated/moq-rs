use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the subscriber to reject an Announce after ANNOUNCE_OK
#[derive(Clone, Debug)]
pub struct AnnounceCancel {
	// Echo back the namespace that was reset
	pub namespace: String,
	// An error code.
	//pub code: u64,

	// An optional, human-readable reason.
	//pub reason: String,
}

impl Decode for AnnounceCancel {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = String::decode(r)?;
		//let code = u64::decode(r)?;
		//let reason = String::decode(r)?;

		Ok(Self {
			namespace,
			//code,
			//reason,
		})
	}
}

impl Encode for AnnounceCancel {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.namespace.encode(w)?;
		//self.code.encode(w)?;
		//self.reason.encode(w)?;

		Ok(())
	}
}
