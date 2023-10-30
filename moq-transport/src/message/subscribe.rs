use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	/// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub id: VarInt,

	/// The track namespace.
	pub namespace: String,

	/// The track name.
	pub name: String,

	/// The start/end group/object.
	pub start_group: SubscribeLocation,
	pub start_object: SubscribeLocation,
	pub end_group: SubscribeLocation,
	pub end_object: SubscribeLocation,

	/// An optional auth token.
	// TODO: > The value is an ASCII string.
	pub auth: String,
}

impl Subscribe {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let namespace = String::decode(r).await?;
		let name = String::decode(r).await?;

		let start_group = SubscribeLocation::decode(r).await?;
		let start_object = SubscribeLocation::decode(r).await?;
		let end_group = SubscribeLocation::decode(r).await?;
		let end_object = SubscribeLocation::decode(r).await?;

		// You can't have a start object without a start group.
		if start_group == SubscribeLocation::None && start_object != SubscribeLocation::None {
			return Err(DecodeError::InvalidSubscribeLocation);
		}

		// You can't have an end object without an end group.
		if end_group == SubscribeLocation::None && end_object != SubscribeLocation::None {
			return Err(DecodeError::InvalidSubscribeLocation);
		}

		// NOTE: There's some more location restrictions in the draft, but they're enforced at a higher level.

		let auth = String::decode(r).await?;

		Ok(Self {
			id,
			namespace,
			name,
			start_group,
			start_object,
			end_group,
			end_object,
			auth,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.namespace.encode(w).await?;
		self.name.encode(w).await?;

		self.start_group.encode(w).await?;
		self.start_object.encode(w).await?;
		self.end_group.encode(w).await?;
		self.end_object.encode(w).await?;

		self.auth.encode(w).await?;

		Ok(())
	}
}

/// Signal where the subscription should begin, relative to the current cache.
#[derive(Clone, Debug, PartialEq)]
pub enum SubscribeLocation {
	None,
	Absolute(VarInt),
	Previous(VarInt),
	Next(VarInt),
}

impl SubscribeLocation {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let kind = VarInt::decode(r).await?;

		match kind.into_inner() {
			0 => Ok(Self::None),
			1 => Ok(Self::Absolute(VarInt::decode(r).await?)),
			2 => Ok(Self::Previous(VarInt::decode(r).await?)),
			3 => Ok(Self::Next(VarInt::decode(r).await?)),
			_ => Err(DecodeError::InvalidSubscribeLocation),
		}
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::None => {
				VarInt::from_u32(0).encode(w).await?;
			}
			Self::Absolute(val) => {
				VarInt::from_u32(1).encode(w).await?;
				val.encode(w).await?;
			}
			Self::Previous(val) => {
				VarInt::from_u32(2).encode(w).await?;
				val.encode(w).await?;
			}
			Self::Next(val) => {
				VarInt::from_u32(3).encode(w).await?;
				val.encode(w).await?;
			}
		}

		Ok(())
	}
}
