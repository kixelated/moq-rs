use tokio::io::{AsyncRead, AsyncWrite};

use crate::coding::{Decode, DecodeError, Encode, EncodeError, Params};
use crate::session::SessionError;
use crate::VarInt;
use paste::paste;

/// This is a custom extension scheme to allow/require draft PRs.
///
/// By convention, the extension number is the PR number + 0xe0000.

macro_rules! extensions {
    {$($name:ident = $val:expr,)*} => {
		#[derive(Clone, Default, Debug)]
		pub struct Extensions {
			$(
				pub $name: bool,
			)*
		}

		impl Extensions {
			pub async fn load(params: &mut Params) -> Result<Self, DecodeError> {
				let mut extensions = Self::default();

				$(
					if let Some(_) = params.get::<ExtensionExists>(VarInt::from_u32($val)).await? {
						extensions.$name = true
					}
				)*

				Ok(extensions)
			}

			pub async fn store(&self, params: &mut Params) -> Result<(), EncodeError> {
				$(
					if self.$name {
						params.set(VarInt::from_u32($val), ExtensionExists{}).await?;
					}
				)*

				Ok(())
			}

			paste! {
				$(
					pub fn [<require_ $name>](&self) -> Result<(), SessionError> {
						match self.$name {
							true => Ok(()),
							false => Err(SessionError::RequiredExtension(VarInt::from_u32($val))),
						}
					}
				)*
			}
		}
	}
}

struct ExtensionExists;

#[async_trait::async_trait]
impl Decode for ExtensionExists {
	async fn decode<R: AsyncRead>(_r: &mut R) -> Result<Self, DecodeError> {
		Ok(ExtensionExists {})
	}
}

#[async_trait::async_trait]
impl Encode for ExtensionExists {
	async fn encode<W: AsyncWrite>(&self, _w: &mut W) -> Result<(), EncodeError> {
		Ok(())
	}
}

extensions! {
	// required for publishers: OBJECT contains expires VarInt in seconds: https://github.com/moq-wg/moq-transport/issues/249
	// TODO write up a PR
	object_expires = 0xe00f9,

	// required: SUBSCRIBE chooses track ID: https://github.com/moq-wg/moq-transport/pull/258
	subscriber_id = 0xe0102,

	// optional: SUBSCRIBE contains namespace/name tuple: https://github.com/moq-wg/moq-transport/pull/277
	subscribe_split = 0xe0115,
}
