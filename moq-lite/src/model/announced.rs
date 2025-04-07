use std::collections::{BTreeSet, VecDeque};
use tokio::sync::mpsc;
use web_async::{Lock, LockWeak};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Announced {
	// The track was announced.
	Active { suffix: String },

	// The track is no longer active.
	Ended { suffix: String },

	// Indicates we're caught up to the current state of the world.
	Live,
}

#[cfg(test)]
impl Announced {
	pub fn assert_active(&self, expected: &str) {
		match self {
			Announced::Active { suffix } => assert_eq!(suffix, expected),
			_ => panic!("expected active announce"),
		}
	}

	pub fn assert_ended(&self, expected: &str) {
		match self {
			Announced::Ended { suffix } => assert_eq!(suffix, expected),
			_ => panic!("expected ended announce"),
		}
	}

	pub fn assert_live(&self) {
		match self {
			Announced::Live => (),
			_ => panic!("expected live announce"),
		}
	}
}

#[derive(Default)]
struct ProducerState {
	active: BTreeSet<String>,
	consumers: Vec<(Lock<ConsumerState>, mpsc::Sender<()>)>,
	live: bool,
}

impl ProducerState {
	fn insert(&mut self, path: String) -> bool {
		if !self.active.insert(path.clone()) {
			return false;
		}

		let mut i = 0;

		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().insert(&path);
				notify.try_send(()).ok();
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}

		true
	}

	fn remove(&mut self, path: &str) -> bool {
		if !self.active.remove(path) {
			return false;
		}

		let mut i = 0;

		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().remove(path);
				notify.try_send(()).ok();
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}

		true
	}

	fn live(&mut self) -> bool {
		if self.live {
			return false;
		}

		self.live = true;

		let mut i = 0;
		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().live();
				notify.try_send(()).ok();
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}

		true
	}

	fn consumer(&mut self, prefix: String) -> ConsumerState {
		let mut added = VecDeque::new();

		for active in &self.active {
			if let Some(suffix) = active.strip_prefix(&prefix) {
				added.push_back(suffix.to_string());
			}
		}

		ConsumerState {
			prefix,
			added,
			removed: VecDeque::new(),
			live: self.live,
		}
	}

	fn subscribe(&mut self, consumer: Lock<ConsumerState>) -> mpsc::Receiver<()> {
		let (tx, rx) = mpsc::channel(1);
		self.consumers.push((consumer.clone(), tx));
		rx
	}
}

impl Drop for ProducerState {
	fn drop(&mut self) {
		for (consumer, notify) in &self.consumers {
			let mut consumer = consumer.lock();
			for path in &self.active {
				consumer.remove(path);
			}

			notify.try_send(()).ok();
		}
	}
}

#[derive(Clone)]
struct ConsumerState {
	prefix: String,
	added: VecDeque<String>,
	removed: VecDeque<String>,
	live: bool,
}

impl ConsumerState {
	pub fn insert(&mut self, path: &str) {
		if let Some(suffix) = path.strip_prefix(&self.prefix) {
			self.removed.retain(|removed| removed != suffix);
			self.added.push_back(suffix.to_string());
		}
	}

	pub fn remove(&mut self, path: &str) {
		if let Some(suffix) = path.strip_prefix(&self.prefix) {
			// Check if we haven't consumed this suffix yet.
			self.added.retain(|added| added != suffix);
			self.removed.push_back(suffix.to_string());
		}
	}

	pub fn live(&mut self) {
		self.live = true;
	}

	pub fn reset(&mut self) {
		self.added.clear();
		self.removed.clear();
		self.live = false;
	}
}

/// Announces tracks to consumers over the network.
// TODO Cloning Producers is questionable. It might be better to chain them (consumer -> producer).
#[derive(Default, Clone)]
pub struct AnnouncedProducer {
	state: Lock<ProducerState>,
}

impl AnnouncedProducer {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a track, returning true if it's new.
	pub fn announce<T: ToString>(&mut self, path: T) -> bool {
		let path = path.to_string();
		let mut state = self.state.lock();
		state.insert(path)
	}

	pub fn announce_parts(&mut self, prefix: &str, suffix: &str) -> bool {
		let path = format!("{}{}", prefix, suffix);
		let mut state = self.state.lock();
		state.insert(path)
	}

	/// Check if a track is active.
	pub fn is_active(&self, path: &str) -> bool {
		self.state.lock().active.contains(path)
	}

	/// Check if any tracks are active.
	pub fn is_empty(&self) -> bool {
		self.state.lock().active.is_empty()
	}

	/// Stop announcing a track, returning true if it was active.
	pub fn unannounce(&mut self, path: &str) -> bool {
		let mut state = self.state.lock();
		state.remove(path)
	}

	pub fn unannounce_parts(&mut self, prefix: &str, suffix: &str) -> bool {
		let path = format!("{}{}", prefix, suffix);
		let mut state = self.state.lock();
		state.remove(&path)
	}

	/// Indicate that we're caught up to the current state of the world.
	pub fn live(&mut self) -> bool {
		let mut state = self.state.lock();
		state.live()
	}

	/// Subscribe to all announced tracks matching the (wildcard) filter.
	pub fn subscribe<S: ToString>(&self, prefix: S) -> AnnouncedConsumer {
		let mut state = self.state.lock();
		let consumer = Lock::new(state.consumer(prefix.to_string()));
		let notify = state.subscribe(consumer.clone());
		AnnouncedConsumer::new(self.state.downgrade(), consumer, notify)
	}

	/// Clear all announced tracks.
	pub fn reset(&mut self) {
		let mut state = self.state.lock();

		let mut i = 0;
		while let Some((consumer, notify)) = state.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().reset();
				i += 1;
			} else {
				state.consumers.swap_remove(i);
			}
		}
	}

	/// Wait until all consumers have been dropped.
	///
	/// NOTE: subscribe can be called to unclose the producer.
	pub async fn closed(&self) {
		// Keep looping until all consumers are closed.
		while let Some(notify) = self.closed_inner() {
			notify.closed().await;
		}
	}

	// Returns the closed notify of any consumer.
	fn closed_inner(&self) -> Option<mpsc::Sender<()>> {
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

/// Consumes announced tracks over the network matching an optional prefix.
pub struct AnnouncedConsumer {
	producer: LockWeak<ProducerState>,
	state: Lock<ConsumerState>,
	notify: mpsc::Receiver<()>,

	// True if we've returned that the track is live.
	live: bool,
}

impl AnnouncedConsumer {
	fn new(producer: LockWeak<ProducerState>, state: Lock<ConsumerState>, notify: mpsc::Receiver<()>) -> Self {
		Self {
			producer,
			state,
			notify,
			live: false,
		}
	}

	/// Returns the next announced track.
	pub async fn next(&mut self) -> Option<Announced> {
		loop {
			{
				let mut state = self.state.lock();

				if let Some(removed) = state.removed.pop_front() {
					return Some(Announced::Ended { suffix: removed });
				}

				if let Some(added) = state.added.pop_front() {
					return Some(Announced::Active { suffix: added });
				}

				if !self.live && state.live {
					self.live = true;
					return Some(Announced::Live);
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
impl Clone for AnnouncedConsumer {
	fn clone(&self) -> Self {
		let consumer = Lock::new(self.state.lock().clone());

		match self.producer.upgrade() {
			Some(producer) => {
				let mut producer = producer.lock();
				let notify = producer.subscribe(consumer.clone());
				AnnouncedConsumer::new(self.producer.clone(), consumer, notify)
			}
			None => {
				let (_, notify) = mpsc::channel(1);
				AnnouncedConsumer::new(self.producer.clone(), consumer, notify)
			}
		}
	}
}

#[cfg(test)]
use futures::FutureExt;

#[cfg(test)]
impl AnnouncedConsumer {
	fn assert_active(&mut self, capture: &str) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_active(capture);
	}

	fn assert_ended(&mut self, capture: &str) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_ended(capture);
	}

	fn assert_wait(&mut self) {
		assert_eq!(self.next().now_or_never(), None);
	}

	fn assert_done(&mut self) {
		assert_eq!(self.next().now_or_never(), Some(None));
	}

	fn assert_live(&mut self) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_live();
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn simple() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		assert!(!producer.is_active("a/b"));
		assert!(producer.announce("a/b"));
		assert!(producer.is_active("a/b"));

		consumer.assert_active("a/b");

		assert!(producer.unannounce("a/b"));
		assert!(!producer.is_active("a/b"));

		consumer.assert_ended("a/b");
		consumer.assert_wait();
	}

	#[test]
	fn multi() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		assert!(producer.announce("a/b"));
		assert!(producer.announce("a/c"));
		assert!(producer.announce("d/e"));

		// Make sure we get all of the paths in order.
		consumer.assert_active("a/b");
		consumer.assert_active("a/c");
		consumer.assert_active("d/e");
		consumer.assert_wait();
	}

	#[test]
	fn late() {
		let mut producer = AnnouncedProducer::new();

		assert!(producer.announce("a/b"));
		assert!(producer.announce("a/c"));

		// Subscribe after announcing.
		let mut consumer = producer.subscribe("");

		assert!(producer.announce("d/e"));
		assert!(producer.announce("d/d"));

		// Make sure we get all of the paths in order.
		consumer.assert_active("a/b");
		consumer.assert_active("a/c");
		consumer.assert_active("d/e");
		consumer.assert_active("d/d");
		consumer.assert_wait();
	}

	#[test]
	fn prefix() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("a/");

		assert!(producer.announce("a/b"));
		assert!(producer.announce("a/c"));
		assert!(producer.announce("d/e"));

		consumer.assert_active("b");
		consumer.assert_active("c");
		consumer.assert_wait();
	}

	#[test]
	fn prefix_unannounce() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("a/");

		assert!(producer.announce("a/b"));
		assert!(producer.announce("a/c"));
		assert!(producer.announce("d/e"));

		consumer.assert_active("b");
		consumer.assert_active("c");
		consumer.assert_wait();

		assert!(producer.unannounce("d/e"));
		assert!(producer.unannounce("a/c"));
		assert!(producer.unannounce("a/b"));

		consumer.assert_ended("c");
		consumer.assert_ended("b");
		consumer.assert_wait();
	}

	#[test]
	fn flicker() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		assert!(!producer.is_active("a/b"));
		assert!(producer.announce("a/b"));
		assert!(producer.is_active("a/b"));
		assert!(producer.unannounce("a/b"));
		assert!(!producer.is_active("a/b"));

		// We missed it.
		consumer.assert_wait();
	}

	#[test]
	fn dropped() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		producer.announce("a/b");
		consumer.assert_active("a/b");
		producer.announce("a/c");
		consumer.assert_active("a/c");

		// Don't consume "d/e" before dropping.
		producer.announce("d/e");
		drop(producer);

		consumer.assert_ended("a/b");
		consumer.assert_ended("a/c");
		consumer.assert_done();
	}

	#[test]
	fn live() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		producer.announce("a/b");
		producer.live();
		producer.announce("a/c");

		consumer.assert_active("a/b");
		consumer.assert_active("a/c");
		// We actually get live after "a/c" because we were slow to consume.
		consumer.assert_live();

		producer.live(); // no-op
		producer.announce("d/e");

		consumer.assert_active("d/e");
		consumer.assert_wait();
	}

	#[tokio::test]
	async fn wakeup() {
		tokio::time::pause();

		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		tokio::spawn(async move {
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce("a/b");
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce("a/c");
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.unannounce("a/b");
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			// Don't actually unannounce p2, just drop.
			drop(producer);
		});

		consumer.next().await.unwrap().assert_active("a/b");
		consumer.next().await.unwrap().assert_active("a/c");
		consumer.next().await.unwrap().assert_ended("a/b");
		consumer.next().await.unwrap().assert_ended("a/c");
		assert_eq!(consumer.next().await, None);
	}
}
