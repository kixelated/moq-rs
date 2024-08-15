use bytes::Bytes;

#[derive(Clone, Debug, PartialEq)]
pub struct Init {
	pub styp: mp4::FtypBox,
	pub moov: mp4::MoovBox,

	pub raw: Bytes,
}
