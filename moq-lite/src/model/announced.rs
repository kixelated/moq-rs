use std::collections::{BTreeSet, VecDeque};
use tokio::sync::mpsc;
use web_async::{Lock, LockWeak};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Announced {
	// The broadcast was announced.
	Start(Broadcast),

	// The broadcast is no longer active.
	End(Broadcast),
}

impl Announced {
	pub fn path(&self) -> &str {
		&self.broadcast().path
	}

	pub fn broadcast(&self) -> &Broadcast {
		match self {
			Announced::Start(broadcast) => &broadcast,
			Announced::End(broadcast) => &broadcast,
		}
	}
}

#[cfg(test)]
impl Announced {
	pub fn assert_active(&self, expected: &Broadcast) {
		match self {
			Announced::Start(broadcast) => assert_eq!(broadcast, expected),
			_ => panic!("expected active announce"),
		}
	}

	pub fn assert_ended(&self, expected: &Broadcast) {
		match self {
			Announced::End(broadcast) => assert_eq!(broadcast, expected),
			_ => panic!("expected ended announce"),
		}
	}
}

#[derive(Default)]
struct ProducerState {
	// A BTreeSet just for ordering so the unit tests are deterministic.
	active: BTreeSet<Broadcast>,
	consumers: Vec<(Lock<ConsumerState>, mpsc::Sender<()>)>,
}

impl ProducerState {
	fn insert(&mut self, broadcast: Broadcast) -> bool {
		let unique = self.active.insert(broadcast.clone());
		if !unique {
			self.update(Announced::End(broadcast.clone()));
		}

		self.update(Announced::Start(broadcast.clone()));
		unique
	}

	fn remove(&mut self, broadcast: &Broadcast) -> bool {
		let existing = self.active.remove(broadcast);
		if existing {
			self.update(Announced::End(broadcast.clone()));
		}

		existing
	}

	fn update(&mut self, update: Announced) {
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
			if active.path.starts_with(&prefix) {
				init.push_back(Announced::Start(active.clone()));
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
			self.update(Announced::End(broadcast.clone()));
		}
	}
}

#[derive(Clone)]
struct ConsumerState {
	prefix: String,
	updates: VecDeque<Announced>,
}

impl ConsumerState {
	pub fn push(&mut self, update: Announced) {
		if update.path().starts_with(&self.prefix) {
			self.updates.push_back(update);
		}
	}
}

/// Announces broadcasts to consumers over the network.
#[derive(Default, Clone)]
pub struct AnnouncedProducer {
	state: Lock<ProducerState>,
}

impl AnnouncedProducer {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a broadcast.
	pub fn insert(&mut self, broadcast: Broadcast) -> bool {
		self.state.lock().insert(broadcast.clone())
	}

	pub fn remove(&mut self, broadcast: &Broadcast) -> bool {
		self.state.lock().remove(broadcast)
	}

	/// Check if a broadcast is active.
	pub fn contains(&self, broadcast: &Broadcast) -> bool {
		self.state.lock().active.contains(broadcast)
	}

	/// Check if any broadcasts are active.
	pub fn is_empty(&self) -> bool {
		self.state.lock().active.is_empty()
	}

	/// Subscribe to all announced tracks matching the prefix.
	pub fn consume<S: ToString>(&self, prefix: S) -> AnnouncedConsumer {
		let mut state = self.state.lock();
		let consumer = Lock::new(state.consume(prefix));
		let notify = state.subscribe(consumer.clone());
		AnnouncedConsumer::new(self.state.downgrade(), consumer, notify)
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
pub struct AnnouncedConsumer {
	producer: LockWeak<ProducerState>,
	state: Lock<ConsumerState>,
	notify: mpsc::Receiver<()>,
}

impl AnnouncedConsumer {
	fn new(producer: LockWeak<ProducerState>, state: Lock<ConsumerState>, notify: mpsc::Receiver<()>) -> Self {
		Self {
			producer,
			state,
			notify,
		}
	}

	/// Returns the next announced track.
	pub async fn next(&mut self) -> Option<Announced> {
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
	pub async fn active(&mut self) -> Option<Broadcast> {
		loop {
			if let Some(Announced::Start(added)) = self.next().await {
				return Some(added);
			}
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

use super::Broadcast;

#[cfg(test)]
impl AnnouncedConsumer {
	fn assert_active(&mut self, broadcast: &Broadcast) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_active(broadcast);
	}

	fn assert_ended(&mut self, broadcast: &Broadcast) {
		self.next()
			.now_or_never()
			.expect("would have blocked")
			.expect("no next announcement")
			.assert_ended(broadcast);
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
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("");
		let ab = Broadcast::new("a/b");

		assert!(!producer.contains(&ab));
		assert!(producer.insert(ab.clone()));
		assert!(producer.contains(&ab));

		consumer.assert_active(&ab);

		assert!(producer.remove(&ab));
		assert!(!producer.contains(&ab));

		consumer.assert_ended(&ab);
		consumer.assert_wait();
	}

	#[test]
	fn duplicate() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("");

		let ab = Broadcast::new("a/b");
		let ab2 = Broadcast::new("a/b");

		assert!(producer.insert(ab.clone()));
		assert!(producer.contains(&ab));

		// Doesn't matter if you use broadcast1 or broadcast2; they have the same path.
		assert!(producer.contains(&ab2));
		consumer.assert_active(&ab2);

		// Duplicate announcement.
		assert!(!producer.insert(ab2.clone()));

		// Automatically insert an end/start pair.
		consumer.assert_ended(&ab);
		consumer.assert_active(&ab2);

		drop(producer);

		consumer.assert_ended(&ab2);
		consumer.assert_done();
	}

	#[test]
	fn multi() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.insert(ab.clone()));
		assert!(producer.insert(ac.clone()));
		assert!(producer.insert(de.clone()));

		// Make sure we get all of the paths in order.
		consumer.assert_active(&ab);
		consumer.assert_active(&ac);
		consumer.assert_active(&de);
		consumer.assert_wait();
	}

	#[test]
	fn late() {
		let mut producer = AnnouncedProducer::new();
		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");
		let dd = Broadcast::new("d/d");

		assert!(producer.insert(ab.clone()));
		assert!(producer.insert(ac.clone()));

		// Subscribe after announcing.
		let mut consumer = producer.consume("");

		assert!(producer.insert(de.clone()));
		assert!(producer.insert(dd.clone()));

		// Make sure we get all of the paths in order.
		consumer.assert_active(&ab);
		consumer.assert_active(&ac);
		consumer.assert_active(&de);
		consumer.assert_active(&dd);
		consumer.assert_wait();
	}

	#[test]
	fn prefix() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("a/");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.insert(ab.clone()));
		assert!(producer.insert(ac.clone()));
		assert!(producer.insert(de.clone()));

		consumer.assert_active(&ab);
		consumer.assert_active(&ac);
		consumer.assert_wait();
	}

	#[test]
	fn prefix_unannounce() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("a/");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.insert(ab.clone()));
		assert!(producer.insert(ac.clone()));
		assert!(producer.insert(de.clone()));

		consumer.assert_active(&ab);
		consumer.assert_active(&ac);
		consumer.assert_wait();

		assert!(producer.remove(&de));
		assert!(producer.remove(&ac));
		assert!(producer.remove(&ab));

		consumer.assert_ended(&ac);
		consumer.assert_ended(&ab);
		consumer.assert_wait();
	}

	#[test]
	fn flicker() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("");
		let ab = Broadcast::new("a/b");

		assert!(!producer.contains(&ab));
		assert!(producer.insert(ab.clone()));
		assert!(producer.contains(&ab));
		assert!(producer.remove(&ab));
		assert!(!producer.contains(&ab));

		// We missed it, but we still get a start/stop pair.
		consumer.assert_active(&ab);
		consumer.assert_ended(&ab);
		consumer.assert_wait();
	}

	#[test]
	fn dropped() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.insert(ab.clone()));
		assert!(producer.insert(ac.clone()));

		consumer.assert_active(&ab);
		consumer.assert_active(&ac);

		// Don't consume "d/e" before dropping.
		producer.insert(de.clone());
		drop(producer);

		consumer.assert_active(&de);
		consumer.assert_ended(&ab);
		consumer.assert_ended(&ac);
		consumer.assert_ended(&de);
		consumer.assert_done();
	}

	#[tokio::test]
	async fn wakeup() {
		tokio::time::pause();

		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.consume("");

		tokio::spawn(async move {
			let ab = Broadcast::new("a/b");
			let ac = Broadcast::new("a/c");

			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.insert(ab.clone());
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.insert(ac.clone());
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.remove(&ab);
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			// Don't actually unannounce a/c, just drop.
			drop(producer);
		});

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");

		consumer.next().await.unwrap().assert_active(&ab);
		consumer.next().await.unwrap().assert_active(&ac);
		consumer.next().await.unwrap().assert_ended(&ab);
		consumer.next().await.unwrap().assert_ended(&ac);
		assert_eq!(consumer.next().await, None);
	}
}
