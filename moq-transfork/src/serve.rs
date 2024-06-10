#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ServeError {
	// TODO stop using?
	#[error("done")]
	Done,

	#[error("cancelled")]
	Cancel,

	#[error("closed, code={0}")]
	Closed(u32),

	#[error("not found")]
	NotFound,

	#[error("duplicate")]
	Duplicate,

	#[error("invalid size")]
	Size,

	#[error("internal error: {0}")]
	Internal(String),
}

impl ServeError {
	pub fn code(&self) -> u32 {
		match self {
			Self::Done => 0,
			Self::Cancel => 1,
			Self::Closed(code) => *code,
			Self::NotFound => 404,
			Self::Duplicate => 409,
			Self::Size => 413,
			Self::Internal(_) => 500,
		}
	}
}
