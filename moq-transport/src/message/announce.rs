use crate::coding::{Decode, Encode, Param, Params, Size, VarInt};
use bytes::{Buf, BufMut, Bytes};

#[derive(Default)]
pub struct Announce {
	// The track namespace
	track_namespace: String,

	// An authentication token, param 0x02
	auth: Param<2, Bytes>,

	// Parameters that we don't recognize.
	unknown: Params,
}

impl Decode for Announce {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r)?;

		let mut auth = Param::new();
		let mut unknown = Params::new();

		while r.has_remaining() {
			// TODO is there some way to peek at this varint? I would like to enforce the correct ID in decode.
			let id = VarInt::decode(r)?;

			match u64::from(id) {
				2 => auth = Param::decode(r)?,
				_ => unknown.decode_param(r)?,
			}
		}

		Ok(Self {
			track_namespace,
			auth,
			unknown,
		})
	}
}

impl Encode for Announce {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_namespace.encode(w)?;
		self.auth.encode(w)?;
		self.unknown.encode(w)?;

		Ok(())
	}
}

impl Size for Announce {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_namespace.size()? + self.auth.size()? + self.unknown.size()?)
	}
}
