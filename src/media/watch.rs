use std::sync::Arc;
use tokio::sync::watch;

#[derive(Default)]
struct State<T> {
	queue: Vec<Arc<T>>,
	closed: bool,
}

impl<T> State<T> {
	pub fn new() -> Self {
		Self {
			queue: Vec::new(),
			closed: false,
		}
	}
}

pub struct Producer<T> {
	sender: watch::Sender<State<T>>,
}

impl<T> Producer<T> {
	pub fn new() -> Self {
		let state = State::new();
		let (sender, _) = watch::channel(state);
		Self { sender }
	}

	pub fn push(&self, value: Arc<T>) {
		self.sender.send_modify(|state| state.queue.push(value));
	}

	pub fn close(&self) {
		self.sender.send_modify(|state| state.closed = true);
	}

	pub fn subscribe(&self) -> Subscriber<T> {
		Subscriber::new(self.sender.subscribe())
	}
}

impl<T> Default for Producer<T> {
	fn default() -> Self {
		Self::new()
	}
}

pub struct Subscriber<T> {
	state: watch::Receiver<State<T>>,
	index: usize,
}

impl<T> Subscriber<T> {
	fn new(state: watch::Receiver<State<T>>) -> Self {
		Self { state, index: 0 }
	}

	pub async fn next(&mut self) -> Option<Arc<T>> {
		let state = self
			.state
			.wait_for(|state| state.closed || self.index < state.queue.len())
			.await
			.expect("publisher dropped without close");

		if self.index < state.queue.len() {
			let element = state.queue[self.index].clone();
			self.index += 1;

			Some(element)
		} else {
			None
		}
	}
}
