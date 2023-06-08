
use tokio::sync::watch;

#[derive(Default)]
struct State<T> {
	queue: Vec<T>,
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

pub struct Producer<T: Clone> {
	sender: watch::Sender<State<T>>,
}

impl<T: Clone> Producer<T> {
	pub fn new() -> Self {
		let state = State::new();
		let (sender, _) = watch::channel(state);
		Self { sender }
	}

	pub fn push(&mut self, value: T) {
		self.sender.send_modify(|state| state.queue.push(value));
	}

	pub fn subscribe(&self) -> Subscriber<T> {
		Subscriber::new(self.sender.subscribe())
	}
}

impl<T: Clone> Default for Producer<T> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T: Clone> Drop for Producer<T> {
	fn drop(&mut self) {
		self.sender.send_modify(|state| state.closed = true);
	}
}

#[derive(Clone)]
pub struct Subscriber<T: Clone> {
	state: watch::Receiver<State<T>>,
	index: usize,
}

impl<T: Clone> Subscriber<T> {
	fn new(state: watch::Receiver<State<T>>) -> Self {
		Self { state, index: 0 }
	}

	pub async fn next(&mut self) -> Option<T> {
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
