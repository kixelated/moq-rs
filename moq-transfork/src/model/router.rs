use std::ops;

use crate::{
	model::{Closed, Produce},
	runtime::{Queue, Watch},
};

pub struct Router<T: Produce> {
	_marker: std::marker::PhantomData<T>,
}

impl<T: Produce> Router<T> {
	pub fn produce() -> (RouterWriter<T>, RouterReader<T>) {
		let queue = Queue::default();

		let writer = RouterWriter::new(queue.split());
		let reader = RouterReader::new(queue);

		(writer, reader)
	}
}

pub struct RouterWriter<T: Produce> {
	queue: Queue<RouterRequest<T>>,
}

impl<T: Produce> RouterWriter<T> {
	fn new(queue: Queue<RouterRequest<T>>) -> Self {
		Self { queue }
	}

	pub async fn requested(&mut self) -> Option<RouterRequest<T>> {
		self.queue.pop().await
	}
}

#[derive(Clone)]
pub struct RouterReader<T: Produce> {
	queue: Queue<RouterRequest<T>>,
}

impl<T: Produce> RouterReader<T> {
	fn new(queue: Queue<RouterRequest<T>>) -> Self {
		Self { queue }
	}

	pub async fn subscribe(&self, info: T) -> Result<T::Reader, Closed> {
		let request = RouterRequest::<T>::new(info);
		if self.queue.push(request.split()).is_err() {
			return Err(Closed::Cancel);
		}

		request.response().await
	}
}

pub struct RouterRequest<T: Produce> {
	pub info: T,
	reply: Watch<Option<Result<T::Reader, Closed>>>,
}

impl<T: Produce> RouterRequest<T> {
	fn new(info: T) -> Self {
		Self {
			info,
			reply: Watch::default(),
		}
	}

	fn split(&self) -> Self {
		Self {
			info: self.info.clone(),
			reply: self.reply.split(),
		}
	}

	pub fn serve(self, reader: T::Reader) {
		if let Some(mut state) = self.reply.lock_mut() {
			state.replace(Ok(reader));
		}
	}

	pub fn produce(self) -> T::Writer {
		let (writer, reader) = self.info.produce();
		if let Some(mut state) = self.reply.lock_mut() {
			state.replace(Ok(reader));
		}
		writer
	}

	pub fn close(self, error: Closed) {
		if let Some(mut state) = self.reply.lock_mut() {
			state.replace(Err(error));
		}
	}

	pub async fn response(self) -> Result<T::Reader, Closed> {
		loop {
			{
				let state = self.reply.lock();
				if let Some(res) = state.clone() {
					return res;
				}

				state.changed().ok_or(Closed::Unknown)?
			}
			.await
		}
	}
}

impl<T: Produce> ops::Deref for RouterRequest<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
