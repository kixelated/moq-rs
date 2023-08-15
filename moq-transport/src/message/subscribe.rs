use crate::coding::{decode_string, encode_string, DecodeError, EncodeError, VarInt};

use webtransport_generic::{RecvStream, SendStream};

#[derive(Debug)]
pub struct Subscribe {
	// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: VarInt,

	// The track namespace.
	pub track_namespace: String,

	// The track name.
	pub track_name: String,
}

impl Subscribe {
	pub async fn decode<R: RecvStream>(r: &mut R) -> Result<Self, DecodeError> {
		let track_id = VarInt::decode(r).await?;
		let track_namespace = decode_string(r).await?;
		let track_name = decode_string(r).await?;

		Ok(Self {
			track_id,
			track_namespace,
			track_name,
		})
	}
}

impl Subscribe {
	pub async fn encode<W: SendStream>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_id.encode(w).await?;
		encode_string(&self.track_namespace, w).await?;
		encode_string(&self.track_name, w).await?;

		Ok(())
	}
}
