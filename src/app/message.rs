use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Message {
	pub init: Option<Init>,
	pub segment: Option<Segment>,
}

#[derive(Serialize, Deserialize)]
pub struct Init {}

#[derive(Serialize, Deserialize)]
pub struct Segment {
	pub track_id: u32,
}

impl Message {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn serialize(&self) -> anyhow::Result<Vec<u8>> {
		let str = serde_json::to_string(self)?;
		let bytes = str.as_bytes();
		let size = bytes.len() + 8;

		let mut out = Vec::with_capacity(size);
		out.extend_from_slice(&(size as u32).to_be_bytes());
		out.extend_from_slice(b"warp");
		out.extend_from_slice(bytes);

		Ok(out)
	}
}
