use std::collections::{BTreeSet, VecDeque};
use tokio::sync::mpsc;
use web_async::{Lock, LockWeak};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Announced {
	// The broadcast was announced.
	Active(Broadcast),

	// The broadcast is no longer active.
	Ended(Broadcast),
}

#[cfg(test)]
impl Announced {
	pub fn assert_active(&self, expected: &Broadcast) {
		match self {
			Announced::Active(broadcast) => assert_eq!(broadcast, expected),
			_ => panic!("expected active announce"),
		}
	}

	pub fn assert_ended(&self, expected: &Broadcast) {
		match self {
			Announced::Ended(broadcast) => assert_eq!(broadcast, expected),
			_ => panic!("expected ended announce"),
		}
	}
}

#[derive(Default)]
struct ProducerState {
	active: BTreeSet<Broadcast>,
	consumers: Vec<(Lock<ConsumerState>, mpsc::Sender<()>)>,
}

impl ProducerState {
	fn insert(&mut self, broadcast: Broadcast) -> bool {
		if !self.active.insert(broadcast.clone()) {
			return false;
		}

		let mut i = 0;

		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().insert(&broadcast);
				notify.try_send(()).ok();
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}

		true
	}

	fn remove(&mut self, broadcast: &Broadcast) -> bool {
		if !self.active.remove(broadcast) {
			return false;
		}

		let mut i = 0;

		while let Some((consumer, notify)) = self.consumers.get(i) {
			if !notify.is_closed() {
				consumer.lock().remove(broadcast);
				notify.try_send(()).ok();
				i += 1;
			} else {
				self.consumers.swap_remove(i);
			}
		}

		true
	}

	fn consume<T: ToString>(&mut self, prefix: T) -> ConsumerState {
		let prefix = prefix.to_string();
		let mut added = VecDeque::new();

		for active in &self.active {
			if active.path.starts_with(&prefix) {
				added.push_back(active.clone());
			}
		}

		ConsumerState {
			prefix,
			added,
			removed: VecDeque::new(),
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
			for broadcast in &self.active {
				consumer.remove(broadcast);
			}

			notify.try_send(()).ok();
		}
	}
}

#[derive(Clone)]
struct ConsumerState {
	prefix: String,
	added: VecDeque<Broadcast>,
	removed: VecDeque<Broadcast>,
}

impl ConsumerState {
	pub fn insert(&mut self, broadcast: &Broadcast) {
		if broadcast.path.starts_with(&self.prefix) {
			// Remove any matches that haven't been consumed yet.
			if let Some(index) = self.removed.iter().position(|removed| removed == broadcast) {
				self.removed.remove(index);
			} else {
				self.added.push_back(broadcast.clone());
			}
		}
	}

	pub fn remove(&mut self, broadcast: &Broadcast) {
		if broadcast.path.starts_with(&self.prefix) {
			// Remove any matches that haven't been consumed yet.
			if let Some(index) = self.added.iter().position(|added| added == broadcast) {
				self.added.remove(index);
			} else {
				self.removed.push_back(broadcast.clone());
			}
		}
	}

	pub fn reset(&mut self) {
		self.added.clear();
		self.removed.clear();
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

	/// Announce a broadcast, returning true if it's new.
	pub fn announce(&mut self, broadcast: Broadcast) -> bool {
		let mut state = self.state.lock();
		state.insert(broadcast)
	}

	/// Check if a broadcast is active.
	pub fn is_active(&self, broadcast: &Broadcast) -> bool {
		self.state.lock().active.contains(broadcast)
	}

	/// Check if any broadcasts are active.
	pub fn is_empty(&self) -> bool {
		self.state.lock().active.is_empty()
	}

	/// Stop announcing a broadcast, returning true if it was active.
	pub fn unannounce(&mut self, broadcast: &Broadcast) -> bool {
		let mut state = self.state.lock();
		state.remove(broadcast)
	}

	/// Subscribe to all announced tracks matching the prefix.
	pub fn subscribe<S: ToString>(&self, prefix: S) -> AnnouncedConsumer {
		let mut state = self.state.lock();
		let consumer = Lock::new(state.consume(prefix));
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

				if let Some(removed) = state.removed.pop_front() {
					return Some(Announced::Ended(removed));
				}

				if let Some(added) = state.added.pop_front() {
					return Some(Announced::Active(added));
				}
			}

			self.notify.recv().await?;
		}
	}

	// A helper to only get active broadcasts.
	pub async fn active(&mut self) -> Option<Broadcast> {
		loop {
			if let Some(Announced::Active(added)) = self.next().await {
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
		let mut consumer = producer.subscribe("");
		let ab = Broadcast::new("a/b");

		assert!(!producer.is_active(&ab));
		assert!(producer.announce(ab.clone()));
		assert!(producer.is_active(&ab));

		consumer.assert_active(&ab);

		assert!(producer.unannounce(&ab));
		assert!(!producer.is_active(&ab));

		consumer.assert_ended(&ab);
		consumer.assert_wait();
	}

	#[test]
	fn duplicate() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		let ab = Broadcast::new("a/b");
		let ab2 = Broadcast::new("a/b");

		assert!(producer.announce(ab.clone()));
		assert!(producer.is_active(&ab));

		// Doesn't matter if you use broadcast1 or broadcast2; they have the same path.
		assert!(producer.is_active(&ab2));
		consumer.assert_active(&ab2);

		// Duplicate announcement.
		assert!(!producer.announce(ab2.clone()));

		assert!(producer.unannounce(&ab));
		assert!(!producer.is_active(&ab));
		assert!(!producer.is_active(&ab2));

		consumer.assert_ended(&ab);
		consumer.assert_wait();
	}

	#[test]
	fn multi() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.announce(ab.clone()));
		assert!(producer.announce(ac.clone()));
		assert!(producer.announce(de.clone()));

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

		assert!(producer.announce(ab.clone()));
		assert!(producer.announce(ac.clone()));

		// Subscribe after announcing.
		let mut consumer = producer.subscribe("");

		assert!(producer.announce(de.clone()));
		assert!(producer.announce(dd.clone()));

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
		let mut consumer = producer.subscribe("a/");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.announce(ab.clone()));
		assert!(producer.announce(ac.clone()));
		assert!(producer.announce(de.clone()));

		consumer.assert_active(&ab);
		consumer.assert_active(&ac);
		consumer.assert_wait();
	}

	#[test]
	fn prefix_unannounce() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("a/");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.announce(ab.clone()));
		assert!(producer.announce(ac.clone()));
		assert!(producer.announce(de.clone()));

		consumer.assert_active(&ab);
		consumer.assert_active(&ac);
		consumer.assert_wait();

		assert!(producer.unannounce(&de));
		assert!(producer.unannounce(&ac));
		assert!(producer.unannounce(&ab));

		consumer.assert_ended(&ac);
		consumer.assert_ended(&ab);
		consumer.assert_wait();
	}

	#[test]
	fn flicker() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");
		let ab = Broadcast::new("a/b");

		assert!(!producer.is_active(&ab));
		assert!(producer.announce(ab.clone()));
		assert!(producer.is_active(&ab));
		assert!(producer.unannounce(&ab));
		assert!(!producer.is_active(&ab));

		// We missed it.
		consumer.assert_wait();
	}

	#[test]
	fn dropped() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		let ab = Broadcast::new("a/b");
		let ac = Broadcast::new("a/c");
		let de = Broadcast::new("d/e");

		assert!(producer.announce(ab.clone()));
		assert!(producer.announce(ac.clone()));

		producer.announce(ab.clone());
		consumer.assert_active(&ab);
		producer.announce(ac.clone());
		consumer.assert_active(&ac);

		// Don't consume "d/e" before dropping.
		producer.announce(de.clone());
		drop(producer);

		consumer.assert_ended(&ab);
		consumer.assert_ended(&ac);
		consumer.assert_done();
	}

	#[tokio::test]
	async fn wakeup() {
		tokio::time::pause();

		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe("");

		tokio::spawn(async move {
			let ab = Broadcast::new("a/b");
			let ac = Broadcast::new("a/c");

			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce(ab.clone());
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce(ac.clone());
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.unannounce(&ab);
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
