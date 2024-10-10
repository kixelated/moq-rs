use crate::catalog;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("transfork error: {0}")]
	Transfork(#[from] moq_transfork::Error),

	#[error("catalog error: {0}")]
	Catalog(#[from] catalog::Error),

	#[error("decode error: {0}")]
	Decode(#[from] moq_transfork::coding::DecodeError),

	#[error("duplicate track")]
	DuplicateTrack,

	#[error("missing track")]
	MissingTrack,
}
