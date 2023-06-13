mod announce;
mod announce_error;
mod announce_ok;
mod go_away;
mod subscribe;
mod subscribe_error;
mod subscribe_ok;

pub use announce::*;
pub use announce_error::*;
pub use announce_ok::*;
pub use go_away::*;
pub use subscribe::*;
pub use subscribe_error::*;
pub use subscribe_ok::*;

use crate::coding::{Decode, Encode, Size, VarInt, WithSize};
use bytes::{Buf, BufMut};

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		pub enum Message {
			$($name($name)),*
		}

		impl Decode for Message {
			fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
				let t = VarInt::decode(r)?;

				Ok(match t.into() {
					$(VarInt($val) => {
						let v = WithSize::decode::<B, $name>(r)?;
						Self::$name(v)
					})*
					_ => anyhow::bail!("invalid message type: {}", t),
				})
			}
		}

		impl Encode for Message {
			fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
				match self {
					$(Self::$name(ref m) => {
						VarInt($val).encode(w)?;
						WithSize::encode(w, m)
					},)*
				}
			}
		}

		impl Size for Message {
			fn size(&self) -> anyhow::Result<usize> {
				Ok(match self {
					$(Self::$name(ref m) => {
						VarInt($val).size()? + WithSize::size(m)?
					},)*
				})
			}
		}
    }
}

// Each message is prefixed with the given VarInt type.
message_types! {
	// Proposal: OBJECT and SETUP as control/data stream headers: https://github.com/moq-wg/moq-transport/issues/138
	Subscribe = 0x03,
	SubscribeOk = 0x04,
	SubscribeError = 0x05,
	Announce = 0x06,
	AnnounceOk = 0x07,
	AnnounceError = 0x08,
	GoAway = 0x10,
}
