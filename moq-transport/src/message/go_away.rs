use crate::coding::{decode_string, encode_string, DecodeError, EncodeError};

use webtransport_generic::{RecvStream, SendStream};

#[derive(Debug)]
pub struct GoAway {
	pub url: String,
}

impl GoAway {
	pub async fn decode<R: RecvStream>(r: &mut R) -> Result<Self, DecodeError> {
		let url = decode_string(r).await?;
		Ok(Self { url })
	}

	pub async fn encode<W: SendStream>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.url, w).await
	}
}
