use crate::coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError, VarInt};
use std::fmt;

use super::{Datagram, Group, Object, Track};

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! header_types {
    {$($name:ident = $val:expr,)*} => {
		/// All supported message types.
		#[derive(Clone)]
		pub enum Header {
			$($name($name)),*
		}

		impl Header {
			pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
				let t = VarInt::decode(r).await?;

				match t.into_inner() {
					$($val => {
						let msg = $name::decode(r).await?;
						Ok(Self::$name(msg))
					})*
					_ => Err(DecodeError::InvalidMessage(t)),
				}
			}

			pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
				match self {
					$(Self::$name(ref m) => {
						VarInt::from_u32($val).encode(w).await?;
						m.encode(w).await
					},)*
				}
			}

			pub fn id(&self) -> VarInt {
				match self {
					$(Self::$name(_) => {
						VarInt::from_u32($val)
					},)*
				}
			}

			pub fn name(&self) -> &'static str {
				match self {
					$(Self::$name(_) => {
						stringify!($name)
					},)*
				}
			}

			pub fn subscribe_id(&self) -> VarInt {
				match self {
					$(Self::$name(o) => o.subscribe_id,)*
				}
			}

			pub fn track_alias(&self) -> VarInt {
				match self {
					$(Self::$name(o) => o.track_alias,)*
				}
			}

			pub fn send_order(&self) -> VarInt {
				match self {
					$(Self::$name(o) => o.send_order,)*
				}
			}
		}

		$(impl From<$name> for Header {
			fn from(m: $name) -> Self {
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
	Datagram = 0x1,
	Group = 0x50,
	Track = 0x51,
}
