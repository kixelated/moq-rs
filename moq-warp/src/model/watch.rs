use std::collections::VecDeque;
use tokio::sync::watch;

#[derive(Default)]
struct State<T> {
	queue: VecDeque<T>,
	drained: usize,
}

impl<T> State<T> {
	fn new() -> Self {
		Self {
			queue: VecDeque::new(),
			drained: 0,
		}
	}

	// Add a new element to the end of the queue.
	fn push(&mut self, t: T) {
		self.queue.push_back(t)
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
	}
}

pub struct Publisher<T: Clone> {
	sender: watch::Sender<State<T>>,
}

impl<T: Clone> Publisher<T> {
	pub fn new() -> Self {
		let state = State::new();
		let (sender, _) = watch::channel(state);
		Self { sender }
	}

	// Push a new element to the end of the queue.
	pub fn push(&mut self, value: T) {
		self.sender.send_modify(|state| state.push(value));
	}

	// Remove any elements from the front of the queue that match the condition.
	pub fn drain<F>(&mut self, f: F)
	where
		F: Fn(&T) -> bool,
	{
		// Use send_if_modified to never notify with the updated state.
		self.sender.send_if_modified(|state| {
			state.drain(f);
			false
		});
	}

	// Subscribe for all NEW updates.
	pub fn subscribe(&self) -> Subscriber<T> {
		let index = self.sender.borrow().queue.len();

		Subscriber {
			state: self.sender.subscribe(),
			index,
		}
	}
}

impl<T: Clone> Default for Publisher<T> {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(Clone)]
pub struct Subscriber<T: Clone> {
	state: watch::Receiver<State<T>>,
	index: usize,
}

impl<T: Clone> Subscriber<T> {
	pub async fn next(&mut self) -> Option<T> {
		// Wait until the queue has a new element or if it's closed.
		let state = self
			.state
			.wait_for(|state| self.index < state.drained + state.queue.len())
			.await;

		let state = match state {
			Ok(state) => state,
			Err(_) => return None, // publisher was dropped
		};

		// If our index is smaller than drained, skip past those elements we missed.
		let index = self.index.saturating_sub(state.drained);

		if index < state.queue.len() {
			// Clone the next element in the queue.
			let element = state.queue[index].clone();

			// Increment our index, relative to drained so we can skip ahead if needed.
			self.index = index + state.drained + 1;

			Some(element)
		} else {
			unreachable!("impossible subscriber state")
		}
	}
}
