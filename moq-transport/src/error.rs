pub trait MoqError: std::error::Error {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u32;
}
