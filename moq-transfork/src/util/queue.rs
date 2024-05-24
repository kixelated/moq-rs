use std::collections::VecDeque;

use super::Watch;

// TODO replace with mpsc or similar
pub struct Queue<T> {
	state: Watch<VecDeque<T>>,
}

impl<T> Clone for Queue<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
		}
	}
}

impl<T> Default for Queue<T> {
	fn default() -> Self {
		Self {
			state: Default::default(),
		}
	}
}

impl<T> Queue<T> {
	pub fn push(&self, item: T) {
		self.state.lock_mut().push_back(item);
	}

	pub async fn pop(&self) -> T {
		loop {
			let notify = {
				let queue = self.state.lock();
				if !queue.is_empty() {
					return queue.into_mut().pop_front().unwrap();
				}
				queue.changed()
			};

			notify.await
		}
	}
}
