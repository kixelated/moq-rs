use std::collections::{HashMap, VecDeque};
use tokio::sync::mpsc;
use web_async::{Lock, LockWeak};

use super::BroadcastConsumer;

#[derive(Default)]
struct ProducerState {
	active: HashMap<String, BroadcastConsumer>,
	consumers: Vec<(Lock<ConsumerState>, mpsc::Sender<()>)>,
}

impl ProducerState {
	fn insert(&mut self, path: String, broadcast: BroadcastConsumer) -> Option<BroadcastConsumer> {
		let mut i = 0;

		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				if consumer.lock().insert(&path, &broadcast) {
					notify.try_send(()).ok();
				}
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}

		self.active.insert(path, broadcast)
	}

	fn consume<T: ToString>(&mut self, prefix: T) -> ConsumerState {
		let prefix = prefix.to_string();
		let mut updates = VecDeque::new();

		for (path, broadcast) in self.active.iter() {
			if let Some(suffix) = path.strip_prefix(&prefix) {
				updates.push_back((suffix.to_string(), broadcast.clone()));
			}
		}

		ConsumerState { prefix, updates }
	}

	fn subscribe(&mut self, consumer: Lock<ConsumerState>) -> mpsc::Receiver<()> {
		let (tx, rx) = mpsc::channel(1);
		self.consumers.push((consumer.clone(), tx));
		rx
	}
}

#[derive(Clone)]
struct ConsumerState {
	prefix: String,
	updates: VecDeque<(String, BroadcastConsumer)>,
}

impl ConsumerState {
	pub fn insert(&mut self, path: &str, consumer: &BroadcastConsumer) -> bool {
		if let Some(suffix) = path.strip_prefix(&self.prefix) {
			self.updates.push_back((suffix.to_string(), consumer.clone()));
			true
		} else {
			false
		}
	}
}

/// Announces broadcasts to consumers over the network.
#[derive(Default, Clone)]
pub struct OriginProducer {
	state: Lock<ProducerState>,
}

impl OriginProducer {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a broadcast, returning true if it was unique.
	pub fn publish<S: ToString>(&mut self, path: S, broadcast: BroadcastConsumer) -> bool {
		let path = path.to_string();
		self.state.lock().insert(path, broadcast).is_none()
	}

	/// Publish all broadcasts from the given origin.
	pub fn publish_all(&mut self, broadcasts: OriginConsumer) {
		self.publish_prefix("", broadcasts);
	}

	/// Publish all broadcasts from the given origin with an optional prefix.
	pub fn publish_prefix(&mut self, prefix: &str, mut broadcasts: OriginConsumer) {
		// Really gross that this just spawns a background task, but I want publishing to be sync.
		let mut this = self.clone();

		// Overkill to avoid allocating a string if the prefix is empty.
		let prefix = match prefix {
			"" => None,
			prefix => Some(prefix.to_string()),
		};

		web_async::spawn(async move {
			while let Some((suffix, broadcast)) = broadcasts.next().await {
				let path = match &prefix {
					Some(prefix) => format!("{}{}", prefix, suffix),
					None => suffix,
				};

				this.publish(path, broadcast);
			}
		});
	}

	/// Get a specific broadcast by name.
	pub fn consume(&self, path: &str) -> Option<BroadcastConsumer> {
		self.state.lock().active.get(path).cloned()
	}

	/// Subscribe to all announced broadcasts.
	pub fn consume_all(&self) -> OriginConsumer {
		self.consume_prefix("")
	}

	/// Subscribe to all announced broadcasts matching the prefix.
	pub fn consume_prefix<S: ToString>(&self, prefix: S) -> OriginConsumer {
		let mut state = self.state.lock();
		let consumer = Lock::new(state.consume(prefix));
		let notify = state.subscribe(consumer.clone());
		OriginConsumer::new(self.state.downgrade(), consumer, notify)
	}

	/// Wait until all consumers have been dropped.
	///
	/// NOTE: subscribe can be called to unclose the producer.
	pub async fn unused(&self) {
		// Keep looping until all consumers are closed.
		while let Some(notify) = self.unused_inner() {
			notify.closed().await;
		}
	}

	// Returns the closed notify of any consumer.
	fn unused_inner(&self) -> Option<mpsc::Sender<()>> {
		let mut state = self.state.lock();

		while let Some((_, notify)) = state.consumers.last() {
			if !notify.is_closed() {
				return Some(notify.clone());
			}

			state.consumers.pop();
		}

		None
	}
}

/// Consumes announced broadcasts matching against an optional prefix.
pub struct OriginConsumer {
	producer: LockWeak<ProducerState>,
	state: Lock<ConsumerState>,
	notify: mpsc::Receiver<()>,
}

impl OriginConsumer {
	fn new(producer: LockWeak<ProducerState>, state: Lock<ConsumerState>, notify: mpsc::Receiver<()>) -> Self {
		Self {
			producer,
			state,
			notify,
		}
	}

	/// Returns the next announced broadcast.
	pub async fn next(&mut self) -> Option<(String, BroadcastConsumer)> {
		loop {
			{
				let mut state = self.state.lock();

				if let Some(update) = state.updates.pop_front() {
					return Some(update);
				}
			}

			self.notify.recv().await?;
		}
	}
}

// ugh
// Cloning consumers is problematic because it encourages idle consumers.
// It's also just a pain in the butt to implement.
// TODO figure out a way to remove this.
impl Clone for OriginConsumer {
	fn clone(&self) -> Self {
		let consumer = Lock::new(self.state.lock().clone());

		match self.producer.upgrade() {
			Some(producer) => {
				let mut producer = producer.lock();
				let notify = producer.subscribe(consumer.clone());
				OriginConsumer::new(self.producer.clone(), consumer, notify)
			}
			None => {
				let (_, notify) = mpsc::channel(1);
				OriginConsumer::new(self.producer.clone(), consumer, notify)
			}
		}
	}
}
