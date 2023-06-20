use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::Notify;

pub type Guard<'a, T> = MutexGuard<'a, State<T>>;
pub type Shared<T> = Arc<Mutex<State<T>>>;

#[derive(Clone)]
pub struct Watch<T> {
	state: Shared<T>,
	index: usize,
}

impl<T> Watch<T> {
	pub fn new(state: Shared<T>) -> Self {
		Self { state, index: 0 }
	}

	// Drain updates from the start that match the conditional.
	pub fn drain(&self, f: impl Fn(&T) -> bool) -> usize {
		let mut state = self.state.lock().unwrap();
		state.drain(&f)
	}
}

impl<T> Watch<T>
where
	T: Clone,
{
	// Lock the state, consuming any updates and invoking the function.
	pub fn lock(&mut self, mut f: impl FnMut(T)) -> Guard<T> {
		let state: Guard<T> = self.state.lock().unwrap();

		loop {
			let index = self.index.saturating_sub(state.drained);

			if index < state.queue.len() {
				// Clone the next element in the queue.
				let element = state.queue[index].clone();

				// Increment our index, relative to drained so we can skip ahead if needed.
				self.index = index + state.drained + 1;

				f(element)
			} else {
				break;
			}
		}

		state
	}

	// Consume the next update, blocking until available.
	pub async fn next(&mut self) -> T {
		loop {
			let notify = {
				let state = self.state.lock().unwrap();
				let index = self.index.saturating_sub(state.drained);

				if index < state.queue.len() {
					// Clone the next element in the queue.
					let element = state.queue[index].clone();

					// Increment our index, relative to drained so we can skip ahead if needed.
					self.index = index + state.drained + 1;

					return element;
				}

				// Return the notify handle and release the lock.
				state.notify.clone()
			};

			// Release the lock and wait for updates.
			notify.notified().await;
		}
	}
}

pub struct State<T> {
	queue: VecDeque<T>,
	drained: usize,
	notify: Arc<Notify>,
}

// TOD why doesn't derive Default work?
impl<T> Default for State<T> {
	fn default() -> Self {
		Self {
			queue: VecDeque::new(),
			drained: 0,
			notify: Arc::new(Notify::new()),
		}
	}
}

impl<T> State<T> {
	fn new() -> Self {
		Self::default()
	}

	// Add a new element to the end of the queue.
	pub fn push(&mut self, t: T) {
		self.queue.push_back(t);
		self.notify.notify_waiters();
	}

	// Remove elements from the head of the queue if they match the conditional.
	fn drain<F>(&mut self, f: F) -> usize
	where
		F: Fn(&T) -> bool,
	{
		let prior = self.drained;

		while let Some(first) = self.queue.front() {
			if !f(first) {
				break;
			}

			self.queue.pop_front();
			self.drained += 1;
		}

		self.drained - prior

		// Don't notify since this is a drain
	}
}
