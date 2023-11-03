pub trait MoqError {
	/// An integer code that is sent over the wire.
	fn code(&self) -> u32;

	/// An optional reason sometimes sent over the wire.
	fn reason(&self) -> String;
}
