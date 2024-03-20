use std::collections::VecDeque;

use super::Watch;

pub struct Queue<T, E: Clone> {
	state: Watch<State<T, E>>,
}

impl<T, E: Clone> Clone for Queue<T, E> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
		}
	}
}

impl<T, E: Clone> Default for Queue<T, E> {
	fn default() -> Self {
		Self {
			state: Default::default(),
		}
	}
}

struct State<T, E: Clone> {
	queue: VecDeque<T>,
	closed: Result<(), E>,
}

impl<T, E: Clone> Default for State<T, E> {
	fn default() -> Self {
		Self {
			queue: Default::default(),
			closed: Ok(()),
		}
	}
}

impl<T, E: Clone> Queue<T, E> {
	pub fn push(&self, item: T) -> Result<(), E> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.queue.push_back(item);
		Ok(())
	}

	pub async fn pop(&self) -> Result<T, E> {
		loop {
			let notify = {
				let state = self.state.lock();
				state.closed.clone()?;

				if !state.queue.is_empty() {
					return Ok(state.into_mut().queue.pop_front().unwrap());
				}
				state.changed()
			};

			notify.await
		}
	}

	pub fn close(&self, err: E) -> Result<(), E> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.closed = Err(err);
		Ok(())
	}
}
