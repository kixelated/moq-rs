/// A type that is split into 1 producer and N consumers.
pub trait Produce: Clone {
	type Consumer: Clone;
	type Producer;

	fn produce(self) -> (Self::Producer, Self::Consumer);
}
