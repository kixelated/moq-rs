mod client;
mod object;
mod server;

pub use client::*;
pub use object::*;
pub use server::*;

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

		// Unwrap the enum into the specified type.
		$(impl TryFrom<Message> for $name {
			type Error = anyhow::Error;

			fn try_from(m: Message) -> Result<Self, Self::Error> {
				match m {
					Message::$name(m) => Ok(m),
					_ => anyhow::bail!("invalid message type"),
				}
			}
		})*
    }
}

// Each message is prefixed with the given VarInt type.
message_types! {
	Object = 0x00,
	Client = 0x01,
	Server = 0x02, // proposal: moq-wg/moq-transport#212
}

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

impl Message {
	pub async fn read<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		// Read the type and the size.
		let ty = VarInt::read(r).await?;
		let size = VarInt::read(r).await?;

		// Limit the reader to the remaining size of the message.
		// TODO support 0
		let mut r: tokio::io::Take<&mut R> = r.take(size.into());

		// Create a new buffer we will use to decode.
		// TODO is there a way to avoid this temporary buffer?
		// I imagine we'll have to change the Decode trait to be AsyncRead
		let mut buf = Vec::new();

		// We have to write the type and size back to the buffer...
		ty.write(&mut buf).await?;
		size.write(&mut buf).await?;

		// Read the rest of the message into the buffer.
		r.read_to_end(&mut buf).await?;

		// Actually decode the message from the buffer.
		Self::decode(&mut buf.as_slice())
	}

	pub async fn write<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		// TODO is there a way to avoid this temporary buffer?
		// I imagine we'll have to change the Encode trait to be AsyncWrite
		let mut buf = Vec::new();
		self.encode(&mut buf)?;
		w.write_all(&buf).await?;

		Ok(())
	}
}
