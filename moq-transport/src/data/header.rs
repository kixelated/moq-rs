use crate::coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError};
use paste::paste;
use std::fmt;

use super::{GroupHeader, ObjectHeader, TrackHeader};

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! header_types {
    {$($name:ident = $val:expr,)*} => {
		/// All supported message types.
		#[derive(Clone)]
		pub enum Header {
			$($name(paste! { [<$name Header>] })),*
		}

		impl Header {
			pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
				let t = u64::decode(r).await?;

				match t {
					$($val => {
						let msg = <paste! { [<$name Header>] }>::decode(r).await?;
						Ok(Self::$name(msg))
					})*
					_ => Err(DecodeError::InvalidMessage(t)),
				}
			}

			pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
				match self {
					$(Self::$name(ref m) => {
						self.id().encode(w).await?;
						m.encode(w).await
					},)*
				}
			}

			pub fn id(&self) -> u64 {
				match self {
					$(Self::$name(_) => {
						$val
					},)*
				}
			}

			pub fn subscribe_id(&self) -> u64 {
				match self {
					$(Self::$name(o) => o.subscribe_id,)*
				}
			}

			pub fn track_alias(&self) -> u64 {
				match self {
					$(Self::$name(o) => o.track_alias,)*
				}
			}

			pub fn send_order(&self) -> u64 {
				match self {
					$(Self::$name(o) => o.send_order,)*
				}
			}
		}

		$(impl From<paste! { [<$name Header>] }> for Header {
			fn from(m: paste! { [<$name Header>] }) -> Self {
				Self::$name(m)
			}
		})*

		impl fmt::Debug for Header {
			// Delegate to the message formatter
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				match self {
					$(Self::$name(ref m) => m.fmt(f),)*
				}
			}
		}
    }
}

// Each object type is prefixed with the given VarInt type.
header_types! {
	Object = 0x0,
	//Datagram = 0x1,
	Group = 0x50,
	Track = 0x51,
}
