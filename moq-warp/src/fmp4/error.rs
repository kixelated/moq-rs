use crate::catalog;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("catalog error: {0}")]
	Catalog(#[from] catalog::Error),

	#[error("transfork error: {0}")]
	Transfork(#[from] moq_transfork::Error),

	#[error("mp4 error: {0}")]
	Mp4(#[from] mp4::Error),

	#[error("missing tracks")]
	MissingTracks,

	#[error("unknown track")]
	UnknownTrack,

	#[error("missing box: {0}")]
	MissingBox(&'static str),

	#[error("duplicate box: {0}")]
	DuplicateBox(&'static str),

	#[error("expected box: {0}")]
	ExpectedBox(&'static str),

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

	#[error("unsupported track: {0}")]
	UnsupportedTrack(&'static str),
}
