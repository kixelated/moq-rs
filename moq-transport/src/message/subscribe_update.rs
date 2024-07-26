use crate::coding::{Decode, DecodeError, Encode, EncodeError, Params};
use crate::message::subscribe::{SubscribeLocation, SubscribePair};
use crate::message::FilterType;

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct SubscribeUpdate {
	/// The subscription ID
	pub id: u64,

	/// Track properties
	pub track_alias: u64, // This alias is useless but part of the spec
	pub track_namespace: String,
	pub track_name: String,

	/// Priorities
	pub subscribe_priority: u8,

	/// Filter type
	pub filter_type: FilterType,

	/// The start/end group/object. (TODO: Make optional)
	pub start: Option<SubscribePair>, // TODO: Make optional
	pub end: Option<SubscribePair>, // TODO: Make optional

	/// Optional parameters
	pub params: Params,
}

impl Decode for SubscribeUpdate {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let track_alias = u64::decode(r)?;
		let track_namespace = String::decode(r)?;
		let track_name = String::decode(r)?;

		let subscribe_priority = u8::decode(r)?;

		let filter_type = FilterType::decode(r)?;

		let start: Option<SubscribePair>;
		let end: Option<SubscribePair>;
		match filter_type {
			FilterType::AbsoluteStart => {
				if r.remaining() < 2 {
					return Err(DecodeError::MissingField);
				}
				start = Some(SubscribePair::decode(r)?);
				end = None;
			}
			FilterType::AbsoluteRange => {
				if r.remaining() < 4 {
					return Err(DecodeError::MissingField);
				}
				start = Some(SubscribePair::decode(r)?);
				end = Some(SubscribePair::decode(r)?);
			}
			_ => {
				start = None;
				end = None;
			}
		}

		if let Some(s) = &start {
			// You can't have a start object without a start group.
			if s.group == SubscribeLocation::None && s.object != SubscribeLocation::None {
				return Err(DecodeError::InvalidSubscribeLocation);
			}
		}
		if let Some(e) = &end {
			// You can't have an end object without an end group.
			if e.group == SubscribeLocation::None && e.object != SubscribeLocation::None {
				return Err(DecodeError::InvalidSubscribeLocation);
			}
		}

		// NOTE: There's some more location restrictions in the draft, but they're enforced at a higher level.

		let params = Params::decode(r)?;

		Ok(Self {
			id,
			track_alias,
			track_namespace,
			track_name,
			subscribe_priority,
			filter_type,
			start,
			end,
			params,
		})
	}
}

impl Encode for SubscribeUpdate {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w)?;
		self.track_alias.encode(w)?;
		self.track_namespace.encode(w)?;
		self.track_name.encode(w)?;

		self.subscribe_priority.encode(w)?;

		self.filter_type.encode(w)?;

		if self.filter_type == FilterType::AbsoluteStart || self.filter_type == FilterType::AbsoluteRange {
			if self.start.is_none() || self.end.is_none() {
				return Err(EncodeError::MissingField);
			}
			if let Some(start) = &self.start {
				start.encode(w)?;
			}
			if let Some(end) = &self.end {
				end.encode(w)?;
			}
		}

		self.params.encode(w)?;

		Ok(())
	}
}
