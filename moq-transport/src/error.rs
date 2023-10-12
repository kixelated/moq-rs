pub trait MoqError {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u32;
	fn reason(&self) -> &str;
}
