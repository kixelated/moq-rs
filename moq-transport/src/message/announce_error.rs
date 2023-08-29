use crate::coding::{decode_string, encode_string, DecodeError, EncodeError, VarInt};

use webtransport_generic::{RecvStream, SendStream};

#[derive(Clone, Debug)]
pub struct AnnounceError {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,
}

impl AnnounceError {
	pub async fn decode<R: RecvStream>(r: &mut R) -> Result<Self, DecodeError> {
		let track_namespace = decode_string(r).await?;
		let code = VarInt::decode(r).await?;
		let reason = decode_string(r).await?;

		Ok(Self {
			track_namespace,
			code,
			reason,
		})
	}

	pub async fn encode<W: SendStream>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.track_namespace, w).await?;
		self.code.encode(w).await?;
		encode_string(&self.reason, w).await?;

		Ok(())
	}
}
