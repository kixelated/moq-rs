use std::ops;

use tokio::sync::{mpsc, oneshot};

use crate::{Error, Produce};

/// Used to respond to arbitrary broadcast/track requests.
pub struct Router<T: Produce> {
	_marker: std::marker::PhantomData<T>,
}

impl<T: Produce> Router<T> {
	pub fn produce() -> (RouterProducer<T>, RouterConsumer<T>) {
		let (send, recv) = mpsc::channel(16);

		let writer = RouterProducer::new(recv);
		let reader = RouterConsumer::new(send);

		(writer, reader)
	}
}

/// Receive broadcast/track requests and return if we can fulfill them.
pub struct RouterProducer<T: Produce> {
	queue: mpsc::Receiver<RouterRequest<T>>,
}

impl<T: Produce> RouterProducer<T> {
	fn new(queue: mpsc::Receiver<RouterRequest<T>>) -> Self {
		Self { queue }
	}

	pub async fn requested(&mut self) -> Option<RouterRequest<T>> {
		self.queue.recv().await
	}
}

/// Subscribe to abitrary broadcast/tracks.
#[derive(Clone)]
pub struct RouterConsumer<T: Produce> {
	queue: mpsc::Sender<RouterRequest<T>>,
}

impl<T: Produce> RouterConsumer<T> {
	fn new(queue: mpsc::Sender<RouterRequest<T>>) -> Self {
		Self { queue }
	}

	pub async fn subscribe(&self, info: T) -> Result<T::Consumer, Error> {
		let (send, recv) = oneshot::channel();
		let request = RouterRequest { info, reply: send };

		if self.queue.send(request).await.is_err() {
			return Err(Error::Cancel);
		}

		recv.await.map_err(|_| Error::Cancel)?
	}
}

/// An outstanding request for a broadcast/track.
pub struct RouterRequest<T: Produce> {
	pub info: T,
	reply: oneshot::Sender<Result<T::Consumer, Error>>,
}

impl<T: Produce> RouterRequest<T> {
	pub fn serve(self, reader: T::Consumer) {
		self.reply.send(Ok(reader)).ok();
	}

	pub fn produce(self) -> T::Producer {
		let (writer, reader) = self.info.produce();
		self.reply.send(Ok(reader)).ok();
		writer
	}

	pub fn close(self, error: Error) {
		self.reply.send(Err(error)).ok();
	}
}

impl<T: Produce> ops::Deref for RouterRequest<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
