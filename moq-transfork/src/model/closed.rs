#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Closed {
	// Cancel is returned when there are no more readers.
	#[error("cancelled")]
	Cancel,

	// The application closes the stream with a code.
	#[error("app code={0}")]
	App(u32),

	#[error("unknown broadcast")]
	UnknownBroadcast,

	#[error("unknown track")]
	UnknownTrack,

	#[error("unknown group")]
	UnknownGroup,

	#[error("unknown subscribe")]
	UnknownSubscribe,

	// The broadcast/track is a duplicate
	#[error("duplicate")]
	Duplicate,
}

impl Closed {
	pub fn code(&self) -> u32 {
		match self {
			Self::Cancel => 0,
			Self::App(code) => *code,
			Self::UnknownBroadcast => 404,
			Self::UnknownTrack => 405,
			Self::UnknownGroup => 406,
			Self::UnknownSubscribe => 407,
			Self::Duplicate => 409,
		}
	}
}
