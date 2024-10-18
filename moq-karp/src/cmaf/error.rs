use crate::{catalog, media};

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("catalog error: {0}")]
	Catalog(#[from] catalog::Error),

	#[error("transfork error: {0}")]
	Transfork(#[from] moq_transfork::Error),

	#[error("mp4 error: {0}")]
	Mp4(#[from] mp4_atom::Error),

	#[error("media error: {0}")]
	Media(#[from] media::Error),

	#[error("missing tracks")]
	MissingTracks,

	#[error("unknown track")]
	UnknownTrack,

	#[error("missing box: {0}")]
	MissingBox(mp4_atom::FourCC),

	#[error("duplicate box: {0}")]
	DuplicateBox(mp4_atom::FourCC),

	#[error("expected box: {0}")]
	ExpectedBox(mp4_atom::FourCC),

	#[error("unexpected box: {0}")]
	UnexpectedBox(mp4_atom::FourCC),

	#[error("unsupported codec: {0}")]
	UnsupportedCodec(&'static str),

	#[error("invalid size")]
	InvalidSize,

	#[error("empty init")]
	EmptyInit,

	#[error("missing init segment")]
	MissingInit,

	#[error("multiple init segments")]
	MultipleInit,

	#[error("trailing data")]
	TrailingData,

	#[error("closed")]
	Closed,

	#[error("unsupported track: {0}")]
	UnsupportedTrack(&'static str),

	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
