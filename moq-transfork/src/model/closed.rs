#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Closed {
	// Cancel is returned when there are no more readers.
	#[error("cancelled")]
	Cancel,

	// The application closes the stream with a code.
	#[error("app code={0}")]
	App(u32),

	#[error("unknown")]
	Unknown,

	// The broadcast/track is a duplicate
	#[error("duplicate")]
	Duplicate,
}

impl Closed {
	pub fn code(&self) -> u32 {
		match self {
			Self::Cancel => 0,
			Self::App(code) => *code,
			Self::Unknown => 404,
			Self::Duplicate => 409,
		}
	}
}
