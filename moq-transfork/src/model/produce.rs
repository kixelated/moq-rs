pub trait Produce: Clone {
	type Reader: Clone;
	type Writer;

	fn produce(self) -> (Self::Writer, Self::Reader);
}
