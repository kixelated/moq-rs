use std::collections::{BTreeSet, VecDeque};
use tokio::sync::mpsc;
use web_async::{Lock, LockWeak};

// Re-export the announce message type.
pub use crate::message::Announce;

#[derive(Default)]
struct ProducerState {
	// A BTreeSet just for ordering so the unit tests are deterministic.
	active: BTreeSet<String>,
	consumers: Vec<(Lock<ConsumerState>, mpsc::Sender<()>)>,
}

impl ProducerState {
	fn insert(&mut self, path: String) -> bool {
		if self.active.contains(&path) {
			self.update(Announce::Ended { suffix: path.clone() });
			self.update(Announce::Active { suffix: path });
			return false;
		}

		self.active.insert(path.clone());
		self.update(Announce::Active { suffix: path });
		true
	}

	fn remove(&mut self, path: &str) -> bool {
		let existing = self.active.remove(path);
		if existing {
			self.update(Announce::Ended {
				suffix: path.to_string(),
			});
		}

		existing
	}

	fn update(&mut self, update: Announce) {
		let mut i = 0;

		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().push(update.clone());
				notify.try_send(()).ok();
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}
	}

	fn consume<T: ToString>(&mut self, prefix: T) -> ConsumerState {
		let prefix = prefix.to_string();
		let mut init = VecDeque::new();

		for active in self.active.iter() {
			if let Some(suffix) = active.strip_prefix(&prefix) {
				init.push_back(Announce::Active {
					suffix: suffix.to_string(),
				});
			}
		}

		ConsumerState { prefix, updates: init }
	}

	fn subscribe(&mut self, consumer: Lock<ConsumerState>) -> mpsc::Receiver<()> {
		let (tx, rx) = mpsc::channel(1);
		self.consumers.push((consumer.clone(), tx));
		rx
	}
}

impl Drop for ProducerState {
	fn drop(&mut self) {
		// Collect because I'm lazy and don't want to deal with the borrow checker.
		while let Some(broadcast) = self.active.pop_first() {
			self.update(Announce::Ended {
				suffix: broadcast.clone(),
			});
		}
	}
}

#[derive(Clone)]
struct ConsumerState {
	prefix: String,
	updates: VecDeque<Announce>,
}

impl ConsumerState {
	pub fn push(&mut self, update: Announce) {
		match update {
			Announce::Active { suffix } => {
				if let Some(suffix) = suffix.strip_prefix(&self.prefix) {
					self.updates.push_back(Announce::Active {
						suffix: suffix.to_string(),
					});
				}
			}
			Announce::Ended { suffix } => {
				if let Some(suffix) = suffix.strip_prefix(&self.prefix) {
					self.updates.push_back(Announce::Ended {
						suffix: suffix.to_string(),
					});
				}
			}
		}
	}
}

/// Announces broadcasts to consumers over the network.
#[derive(Default, Clone)]
pub struct AnnounceProducer {
	state: Lock<ProducerState>,
}

impl AnnounceProducer {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a broadcast.
	pub fn insert<T: ToString>(&mut self, path: T) -> bool {
		self.state.lock().insert(path.to_string())
	}

	pub fn remove(&mut self, path: &str) -> bool {
		self.state.lock().remove(path)
	}

	/// Check if a broadcast is active.
	pub fn contains(&self, path: &str) -> bool {
		self.state.lock().active.contains(path)
	}

	/// Check if any broadcasts are active.
	pub fn is_empty(&self) -> bool {
		self.state.lock().active.is_empty()
	}

	/// Subscribe to all announced tracks matching the prefix.
	///
	/// There will be an event each time a suffix starts and later ends.
	pub fn consume<S: ToString>(&self, prefix: S) -> AnnounceConsumer {
		let mut state = self.state.lock();
		let consumer = Lock::new(state.consume(prefix));
		let notify = state.subscribe(consumer.clone());
		AnnounceConsumer::new(self.state.downgrade(), consumer, notify)
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

/// Consumes announced tracks over the network matching an optional prefix.
pub struct AnnounceConsumer {
	producer: LockWeak<ProducerState>,
	state: Lock<ConsumerState>,
	notify: mpsc::Receiver<()>,
}

impl AnnounceConsumer {
	fn new(producer: LockWeak<ProducerState>, state: Lock<ConsumerState>, notify: mpsc::Receiver<()>) -> Self {
		Self {
			producer,
			state,
			notify,
		}
	}

	/// Returns the next announced track.
	pub async fn next(&mut self) -> Option<Announce> {
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

	/// A helper to only get active broadcasts.
	///
	/// You can learn if a track has ended via its `closed` method.
	pub async fn active(&mut self) -> Option<String> {
		loop {
			if let Some(Announce::Active { suffix }) = self.next().await {
				return Some(suffix);
			}
		}
	}
}

// ugh
// Cloning consumers is problematic because it encourages idle consumers.
// It's also just a pain in the butt to implement.
// TODO figure out a way to remove this.
impl Clone for AnnounceConsumer {
	fn clone(&self) -> Self {
		let consumer = Lock::new(self.state.lock().clone());

		match self.producer.upgrade() {
			Some(producer) => {
				let mut producer = producer.lock();
				let notify = producer.subscribe(consumer.clone());
				AnnounceConsumer::new(self.producer.clone(), consumer, notify)
			}
			None => {
				let (_, notify) = mpsc::channel(1);
				AnnounceConsumer::new(self.producer.clone(), consumer, notify)
			}
		}
	}
}

#[cfg(test)]
use futures::FutureExt;

#[cfg(test)]
impl AnnounceConsumer {
	fn assert_active(&mut self, suffix: &str) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_active(suffix);
	}

	fn assert_ended(&mut self, suffix: &str) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_ended(suffix);
	}

	fn assert_wait(&mut self) {
		assert_eq!(self.next().now_or_never(), None);
	}

	fn assert_done(&mut self) {
		assert_eq!(self.next().now_or_never(), Some(None));
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn simple() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("");
		let ab = "a/b";

		assert!(!producer.contains(ab));
		assert!(producer.insert(ab));
		assert!(producer.contains(ab));

		consumer.assert_active(ab);

		assert!(producer.remove(ab));
		assert!(!producer.contains(ab));

		consumer.assert_ended(ab);
		consumer.assert_wait();
	}

	#[test]
	fn duplicate() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("");

		let ab = "a/b";
		let ab2 = "a/b";

		assert!(producer.insert(ab));
		assert!(producer.contains(ab));

		// Doesn't matter if you use broadcast1 or broadcast2; they have the same path.
		assert!(producer.contains(ab2));
		consumer.assert_active(ab2);

		// Duplicate announcement.
		assert!(!producer.insert(ab2));

		// Automatically insert an end/start pair.
		consumer.assert_ended(ab);
		consumer.assert_active(ab2);

		drop(producer);

		consumer.assert_ended(ab2);
		consumer.assert_done();
	}

	#[test]
	fn multi() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("");

		let ab = "a/b";
		let ac = "a/c";
		let de = "d/e";

		assert!(producer.insert(ab));
		assert!(producer.insert(ac));
		assert!(producer.insert(de));

		// Make sure we get all of the paths in order.
		consumer.assert_active(ab);
		consumer.assert_active(ac);
		consumer.assert_active(de);
		consumer.assert_wait();
	}

	#[test]
	fn late() {
		let mut producer = AnnounceProducer::new();
		let ab = "a/b";
		let ac = "a/c";
		let de = "d/e";
		let dd = "d/d";

		assert!(producer.insert(ab));
		assert!(producer.insert(ac));

		// Subscribe after announcing.
		let mut consumer = producer.consume("");

		assert!(producer.insert(de));
		assert!(producer.insert(dd));

		// Make sure we get all of the paths in order.
		consumer.assert_active(ab);
		consumer.assert_active(ac);
		consumer.assert_active(de);
		consumer.assert_active(dd);
		consumer.assert_wait();
	}

	#[test]
	fn prefix() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("a/");

		let ab = "a/b";
		let ac = "a/c";
		let de = "d/e";

		assert!(producer.insert(ab));
		assert!(producer.insert(ac));
		assert!(producer.insert(de));

		consumer.assert_active("b");
		consumer.assert_active("c");
		consumer.assert_wait();
	}

	#[test]
	fn prefix_unannounce() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("a/");

		let ab = "a/b";
		let ac = "a/c";
		let de = "d/e";

		assert!(producer.insert(ab));
		assert!(producer.insert(ac));
		assert!(producer.insert(de));

		consumer.assert_active("b");
		consumer.assert_active("c");
		consumer.assert_wait();

		assert!(producer.remove(de));
		assert!(producer.remove(ac));
		assert!(producer.remove(ab));

		consumer.assert_ended("c");
		consumer.assert_ended("b");
		consumer.assert_wait();
	}

	#[test]
	fn flicker() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("");
		let ab = "a/b";

		assert!(!producer.contains(ab));
		assert!(producer.insert(ab));
		assert!(producer.contains(ab));
		assert!(producer.remove(ab));
		assert!(!producer.contains(ab));

		// We missed it, but we still get a start/stop pair.
		consumer.assert_active(ab);
		consumer.assert_ended(ab);
		consumer.assert_wait();
	}

	#[test]
	fn dropped() {
		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("");

		let ab = "a/b";
		let ac = "a/c";
		let de = "d/e";

		assert!(producer.insert(ab));
		assert!(producer.insert(ac));

		consumer.assert_active(ab);
		consumer.assert_active(ac);

		// Don't consume "d/e" before dropping.
		producer.insert(de);
		drop(producer);

		consumer.assert_active(de);
		consumer.assert_ended(ab);
		consumer.assert_ended(ac);
		consumer.assert_ended(de);
		consumer.assert_done();
	}

	#[tokio::test]
	async fn wakeup() {
		tokio::time::pause();

		let mut producer = AnnounceProducer::new();
		let mut consumer = producer.consume("");

		tokio::spawn(async move {
			let ab = "a/b";
			let ac = "a/c";

			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.insert(ab);
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.insert(ac);
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.remove(ab);
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			// Don't actually unannounce a/c, just drop.
			drop(producer);
		});

		let ab = "a/b";
		let ac = "a/c";

		consumer.next().await.unwrap().assert_active(ab);
		consumer.next().await.unwrap().assert_active(ac);
		consumer.next().await.unwrap().assert_ended(ab);
		consumer.next().await.unwrap().assert_ended(ac);
		assert_eq!(consumer.next().await, None);
	}
}
