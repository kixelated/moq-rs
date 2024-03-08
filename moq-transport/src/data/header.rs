use crate::coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError, VarInt};
use std::fmt;

use super::{Group, Object, Track};

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

			pub fn subscribe(&self) -> VarInt {
				match self {
					$(Self::$name(o) => o.subscribe,)*
				}
			}

			pub fn track(&self) -> VarInt {
				match self {
					$(Self::$name(o) => o.track,)*
				}
			}

			pub fn priority(&self) -> u32 {
				match self {
					$(Self::$name(o) => o.priority,)*
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
	Group = 0x50,
	Track = 0x51,
}
