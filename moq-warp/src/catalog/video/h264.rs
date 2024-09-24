use bytes::{Bytes, BytesMut};
use mp4_atom::Encode;
use serde::{Deserialize, Serialize};
use serde_with::hex::Hex;

#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct H264 {
	pub profile: u8,
	pub constraints: u8,
	pub level: u8,

	#[serde_as(as = "Hex")]
	pub sps: Bytes,

	#[serde_as(as = "Hex")]
	pub pps: Bytes,
}

impl H264 {
	// AVCDecoderConfigurationRecord without the header
	pub fn description(&self) -> Bytes {
		let mut buf = BytesMut::new();

		mp4_atom::Avcc {
			configuration_version: 1,
			avc_profile_indication: self.profile,
			profile_compatibility: self.constraints,
			avc_level_indication: self.level,
			length_size_minus_one: 3, // Is this correct to hard-code?

			sequence_parameter_sets: vec![self.sps.clone()],
			picture_parameter_sets: vec![self.pps.clone()],
		}
		.encode(&mut buf)
		.unwrap();

		buf.freeze().slice(8..)
	}
}

impl std::fmt::Display for H264 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "avc1.{:02x}{:02x}{:02x}", self.profile, self.constraints, self.level)
	}
}

/*
impl std::str::FromStr for H264 {
	type Err = CodecError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut parts = s.split('.');
		if parts.next() != Some("avc1") {
			return Err(CodecError::Invalid);
		}

		let part = parts.next().ok_or(CodecError::Invalid)?;
		if part.len() != 6 {
			return Err(CodecError::Invalid);
		}

		Ok(Self {
			profile: u8::from_str_radix(&part[0..2], 16)?,
			constraints: u8::from_str_radix(&part[2..4], 16)?,
			level: u8::from_str_radix(&part[4..6], 16)?,
			sps: Bytes::new(),
			pps: Bytes::new(),
		})
	}
}
*/

#[cfg(test)]
mod tests {
	/*
	#[test]
	fn test_h264() {
		let encoded = "avc1.42c01e";
		let decoded = H264 {
			profile: 0x42,
			constraints: 0xc0,
			level: 0x1e,
			sps: Vec::new(),
			pps: Vec::new(),
		};

		let output = H264::from_str(encoded).expect("failed to parse");
		assert_eq!(output, decoded);

		let output = decoded.to_string();
		assert_eq!(output, encoded);
	}
	*/
}
