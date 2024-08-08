pub trait Produce: Clone {
	type Consumer: Clone;
	type Producer;

	fn produce(self) -> (Self::Producer, Self::Consumer);
}
