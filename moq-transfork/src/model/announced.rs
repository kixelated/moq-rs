use std::collections::BTreeSet;
use tokio::sync::watch;

/// The suffix of each announced track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Announced {
	// Indicates the track is active.
	Active(String),

	// Indicates the track is no longer active.
	Ended(String),

	// Indicates we're caught up to the current state of the world.
	Live,
}

#[derive(Default)]
struct State {
	active: BTreeSet<String>,
	live: bool,
}

/// Announces tracks to consumers over the network.
#[derive(Clone, Default)]
pub struct AnnouncedProducer {
	state: watch::Sender<State>,
}

impl AnnouncedProducer {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a track, returning true if it's new.
	pub fn announce<T: ToString>(&mut self, path: T) -> bool {
		self.state
			.send_if_modified(|state| state.active.insert(path.to_string()))
	}

	/// Stop announcing a track, returning true if it was active.
	pub fn unannounce<T: AsRef<str>>(&mut self, path: T) -> bool {
		self.state.send_if_modified(|state| state.active.remove(path.as_ref()))
	}

	pub fn is_active<T: AsRef<str>>(&self, path: T) -> bool {
		self.state.borrow().active.contains(path.as_ref())
	}

	/// Indicate that we're caught up to the current state of the world.
	pub fn live(&mut self) -> bool {
		self.state.send_if_modified(|state| {
			let prev = state.live;
			state.live = true;
			!prev
		})
	}

	pub fn is_live(&self) -> bool {
		self.state.borrow().live
	}

	/// Subscribe to all announced tracks, including those already active.
	pub fn subscribe(&self) -> AnnouncedConsumer {
		AnnouncedConsumer::new(self.state.subscribe())
	}

	pub fn len(&self) -> usize {
		self.state.borrow().active.len()
	}

	pub fn is_closed(&self) -> bool {
		self.state.is_closed() && self.state.borrow().active.is_empty()
	}

	pub async fn closed(&self) {
		self.state.closed().await;
	}
}

/// Consumes announced tracks over the network matching an optional prefix.
#[derive(Clone)]
pub struct AnnouncedConsumer {
	// The official list of active paths.
	state: watch::Receiver<State>,

	// A set of updates that we haven't consumed yet.
	active: BTreeSet<String>,

	// Indicates if the publisher is still active.
	live: bool,
}

impl AnnouncedConsumer {
	fn new(state: watch::Receiver<State>) -> Self {
		Self {
			state,
			active: BTreeSet::new(),
			live: false,
		}
	}

	/// Returns the suffix of the next announced track received already.
	fn try_next(&mut self) -> Option<Announced> {
		let state = self.state.borrow();

		// TODO this absolutely should be optimized one day.
		while let Some(removed) = self.active.difference(&state.active).next().cloned() {
			self.active.remove(&removed);
			return Some(Announced::Ended(removed));
		}

		while let Some(added) = state.active.difference(&self.active).next().cloned() {
			self.active.insert(added.clone());
			return Some(Announced::Active(added));
		}

		// Return the live marker if needed.
		if state.live && !self.live {
			self.live = true;
			return Some(Announced::Live);
		}

		None
	}

	/// Returns the suffix of the next announced track.
	pub async fn next(&mut self) -> Option<Announced> {
		// NOTE: This just checks if the producer has been dropped.
		// We're not actually using the `changed()` state properly.
		while self.state.has_changed().is_ok() {
			if let Some(announced) = self.try_next() {
				return Some(announced);
			}

			// Wait for any updates.
			match self.state.changed().await {
				Ok(_) => continue,
				Err(_) => break,
			}
		}

		// The publisher is closed, so return `Ended` for all active paths.
		self.active.pop_first().map(Announced::Ended)
	}
}

#[cfg(test)]
mod test {
	use futures::FutureExt;
	use std::collections::HashSet;

	use super::*;

	#[test]
	fn simple() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		assert!(!producer.is_active("a"));
		assert!(producer.announce("a"));
		assert!(producer.is_active("a"));

		let announced = consumer.next().now_or_never().unwrap().unwrap();
		assert!(matches!(announced, Announced::Active(active) if active == "a"));

		assert!(producer.unannounce("a"));
		assert!(!producer.is_active("a"));

		let announced = consumer.next().now_or_never().unwrap().unwrap();
		assert!(matches!(announced, Announced::Ended(active) if active == "a"));

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn multi() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let mut paths: HashSet<String> = HashSet::from_iter(["a", "b", "c"].map(String::from));
		for path in &paths {
			assert!(!producer.is_active(path));
			assert!(producer.announce(path.clone()));
			assert!(producer.is_active(path));
		}

		// Make sure we get all of the paths only once.
		while !paths.is_empty() {
			let res = consumer.next().now_or_never().unwrap().unwrap();
			match res {
				Announced::Active(active) => assert!(paths.remove(&active)),
				_ => panic!("unexpected announcement: {:?}", res),
			}
		}

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn late() {
		let mut producer = AnnouncedProducer::new();

		let mut paths: HashSet<String> = HashSet::from_iter(["a", "b", "c"].map(String::from));
		for path in &paths {
			assert!(!producer.is_active(path));
			assert!(producer.announce(path.clone()));
			assert!(producer.is_active(path));
		}

		// Subscribe after announcing.
		let mut consumer = producer.subscribe();

		// Make sure we get all of the paths only once.
		while !paths.is_empty() {
			let res = consumer.next().now_or_never().unwrap().unwrap();
			match res {
				Announced::Active(active) => assert!(paths.remove(&active)),
				_ => panic!("unexpected announcement: {:?}", res),
			}
		}

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn flicker() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let path = "a".to_string();

		assert!(!producer.is_active(&path));
		assert!(producer.announce(path.clone()));
		assert!(producer.is_active(&path));
		assert!(producer.unannounce("a"));
		assert!(!producer.is_active(&path));

		// We missed it.
		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn dropped() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		producer.announce("a");
		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active("a".to_string()))
		);
		producer.announce("b");
		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active("b".to_string()))
		);

		// Don't consume "c" before dropping.
		producer.announce("c");
		drop(producer);

		let res = match consumer.next().now_or_never().unwrap().unwrap() {
			Announced::Ended(ended) if ended == "a" || ended == "b" => ended,
			res => panic!("unexpected announcement: {:?}", res),
		};

		match consumer.next().now_or_never().unwrap().unwrap() {
			Announced::Ended(res1) if res1 == res => panic!("duplicate announcement: {:?}", res1),
			Announced::Ended(ended) if ended == "a" || ended == "b" => ended,
			res => panic!("unexpected announcement: {:?}", res),
		};

		// Since the producer is dropped, we immediately return None.
		assert_eq!(consumer.next().now_or_never().unwrap(), None);
	}

	#[test]
	fn live() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		producer.announce("a");
		producer.live();
		producer.announce("b");

		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active("a".to_string()))
		);
		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active("b".to_string()))
		);
		// We actually get live after "b" because we were slow to consume.
		assert_eq!(consumer.next().now_or_never().unwrap(), Some(Announced::Live));

		producer.live(); // no-op
		producer.announce("c");

		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active("c".to_string()))
		);
		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[tokio::test]
	async fn wakeup() {
		tokio::time::pause();

		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		tokio::spawn(async move {
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce("a");
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce("b");
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.unannounce("a");
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			// Don't actually unannounce "b", just drop.
			drop(producer);
		});

		let res = match consumer.next().await.unwrap() {
			Announced::Active(active) if active == "a" || active == "b" => active,
			res => panic!("unexpected announcement: {:?}", res),
		};

		match consumer.next().await.unwrap() {
			Announced::Active(dup) if dup == res => panic!("duplicate announcement: {:?}", dup),
			Announced::Active(active) if active == "a" || active == "b" => active,
			res => panic!("unexpected announcement: {:?}", res),
		};

		assert_eq!(consumer.next().await.unwrap(), Announced::Ended("a".to_string()));
		assert_eq!(consumer.next().await.unwrap(), Announced::Ended("b".to_string()));
		assert_eq!(consumer.next().await, None);
	}
}
