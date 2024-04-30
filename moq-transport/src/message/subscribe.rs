use crate::coding::{Decode, DecodeError, Encode, EncodeError, Params};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	/// The subscription ID
	pub id: u64,

	/// Track properties
	pub track_alias: u64, // This alias is useless but part of the spec
	pub track_namespace: String,
	pub track_name: String,

	/// The start/end group/object.
	pub start: SubscribePair,
	pub end: SubscribePair,

	/// Optional parameters
	pub params: Params,
}

impl Decode for Subscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let track_alias = u64::decode(r)?;
		let track_namespace = String::decode(r)?;
		let track_name = String::decode(r)?;

		let start = SubscribePair::decode(r)?;
		let end = SubscribePair::decode(r)?;

		// You can't have a start object without a start group.
		if start.group == SubscribeLocation::None && start.object != SubscribeLocation::None {
			return Err(DecodeError::InvalidSubscribeLocation);
		}

		// You can't have an end object without an end group.
		if end.group == SubscribeLocation::None && end.object != SubscribeLocation::None {
			return Err(DecodeError::InvalidSubscribeLocation);
		}

		// NOTE: There's some more location restrictions in the draft, but they're enforced at a higher level.

		let params = Params::decode(r)?;

		Ok(Self {
			id,
			track_alias,
			track_namespace,
			track_name,
			start,
			end,
			params,
		})
	}
}

impl Encode for Subscribe {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w)?;
		self.track_alias.encode(w)?;
		self.track_namespace.encode(w)?;
		self.track_name.encode(w)?;

		self.start.encode(w)?;
		self.end.encode(w)?;

		self.params.encode(w)?;

		Ok(())
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct SubscribePair {
	pub group: SubscribeLocation,
	pub object: SubscribeLocation,
}

impl Decode for SubscribePair {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			group: SubscribeLocation::decode(r)?,
			object: SubscribeLocation::decode(r)?,
		})
	}
}

impl Encode for SubscribePair {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.group.encode(w)?;
		self.object.encode(w)?;
		Ok(())
	}
}

/// Signal where the subscription should begin, relative to the current cache.
#[derive(Clone, Debug, PartialEq)]
pub enum SubscribeLocation {
	None,
	Absolute(u64),
	Latest(u64),
	Future(u64),
}

impl Decode for SubscribeLocation {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let kind = u64::decode(r)?;

		match kind {
			0 => Ok(Self::None),
			1 => Ok(Self::Absolute(u64::decode(r)?)),
			2 => Ok(Self::Latest(u64::decode(r)?)),
			3 => Ok(Self::Future(u64::decode(r)?)),
			_ => Err(DecodeError::InvalidSubscribeLocation),
		}
	}
}

impl Encode for SubscribeLocation {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id().encode(w)?;

		match self {
			Self::None => Ok(()),
			Self::Absolute(val) => val.encode(w),
			Self::Latest(val) => val.encode(w),
			Self::Future(val) => val.encode(w),
		}
	}
}

impl SubscribeLocation {
	fn id(&self) -> u64 {
		match self {
			Self::None => 0,
			Self::Absolute(_) => 1,
			Self::Latest(_) => 2,
			Self::Future(_) => 3,
		}
	}
}
