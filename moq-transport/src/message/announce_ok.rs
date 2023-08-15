use crate::coding::{decode_string, encode_string, DecodeError, EncodeError};

use webtransport_generic::{RecvStream, SendStream};

#[derive(Debug)]
pub struct AnnounceOk {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,
}

impl AnnounceOk {
	pub async fn decode<R: RecvStream>(r: &mut R) -> Result<Self, DecodeError> {
		let track_namespace = decode_string(r).await?;
		Ok(Self { track_namespace })
	}

	pub async fn encode<W: SendStream>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.track_namespace, w).await
	}
}
