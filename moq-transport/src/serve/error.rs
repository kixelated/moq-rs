#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ServeError {
	#[error("done")]
	Done,

	#[error("closed, code={0}")]
	Closed(u64),

	#[error("not found")]
	NotFound,

	#[error("duplicate")]
	Duplicate,

	#[error("multiple stream modes")]
	Mode,

	#[error("wrong size")]
	WrongSize,
}

impl ServeError {
	pub fn code(&self) -> u64 {
		match self {
			Self::Done => 0,
			Self::Closed(code) => *code,
			Self::NotFound => 404,
			Self::Duplicate => 409,
			Self::Mode => 400,
			Self::WrongSize => 413,
		}
	}
}
