use std::sync::{
	atomic::{self, AtomicUsize},
	Arc,
};

use tokio::sync::Notify;

#[derive(Default)]
struct State {
	consumers: AtomicUsize,
	producers: AtomicUsize,
	notify: Notify,
}

pub struct Producer {
	state: Arc<State>,
}

impl Producer {
	pub fn subscribe(&self) -> Consumer {
		Consumer::new(self.state.clone())
	}

	pub async fn unused(&self) {
		while self.state.consumers.load(atomic::Ordering::Relaxed) > 0 {
			self.state.notify.notified().await;
		}
	}
}

impl Default for Producer {
	fn default() -> Self {
		let state = State::default();
		state.producers.fetch_add(1, atomic::Ordering::Relaxed);

		Self { state: Arc::new(state) }
	}
}

impl Clone for Producer {
	fn clone(&self) -> Self {
		self.state.producers.fetch_add(1, atomic::Ordering::Relaxed);
		Self {
			state: self.state.clone(),
		}
	}
}

pub struct Consumer {
	state: Arc<State>,
}

impl Consumer {
	fn new(state: Arc<State>) -> Self {
		state.consumers.fetch_add(1, atomic::Ordering::Relaxed);
		Self { state }
	}

	/* TODO uncomment when needed
	pub async fn closed(&self) {
		while self.state.producers.load(atomic::Ordering::Relaxed) > 0 {
			self.state.notify.notified().await;
		}
	}
	*/
}

impl Clone for Consumer {
	fn clone(&self) -> Self {
		Self::new(self.state.clone())
	}
}

impl Drop for Consumer {
	fn drop(&mut self) {
		self.state.consumers.fetch_sub(1, atomic::Ordering::Relaxed);
	}
}
