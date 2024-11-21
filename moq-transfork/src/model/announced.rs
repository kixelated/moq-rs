use std::collections::BTreeSet;
use tokio::sync::watch;

use super::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Announced {
	Active(Path),
	Ended(Path),
}

impl Announced {
	pub fn path(&self) -> &Path {
		match self {
			Announced::Active(path) => path,
			Announced::Ended(path) => path,
		}
	}

	pub fn active(&self) -> Option<&Path> {
		match self {
			Announced::Active(path) => Some(path),
			Announced::Ended(_) => None,
		}
	}

	pub fn ended(&self) -> Option<&Path> {
		match self {
			Announced::Active(_) => None,
			Announced::Ended(path) => Some(path),
		}
	}
}

/// Announces tracks to consumers over the network.
#[derive(Clone, Default)]
pub struct AnnouncedProducer {
	state: watch::Sender<BTreeSet<Path>>,
}

impl AnnouncedProducer {
	pub fn new() -> Self {
		Self::default()
	}

	/// Announce a track, returning true if it's new.
	pub fn announce(&mut self, path: Path) -> bool {
		self.state.send_if_modified(|state| state.insert(path))
	}

	/// Stop announcing a track, returning true if it was active.
	pub fn unannounce(&mut self, path: &Path) -> bool {
		self.state.send_if_modified(|state| state.remove(path))
	}

	pub fn is_active(&self, path: &Path) -> bool {
		self.state.borrow().contains(path)
	}

	/// Subscribe to all announced tracks, including those already active.
	pub fn subscribe(&self) -> AnnouncedConsumer {
		self.subscribe_prefix(Path::default())
	}

	/// Subscribe to all announced tracks based on a prefix, including those already active.
	pub fn subscribe_prefix(&self, prefix: Path) -> AnnouncedConsumer {
		AnnouncedConsumer::new(self.state.subscribe(), prefix)
	}

	pub async fn closed(&self) {
		self.state.closed().await;
	}
}

/// Consumes announced tracks over the network matching an optional prefix.
pub struct AnnouncedConsumer {
	// The official list of active paths.
	state: watch::Receiver<BTreeSet<Path>>,

	// A set of updates that we haven't consumed yet.
	active: BTreeSet<Path>,

	// Only consume paths with this prefix.
	prefix: Path,
}

impl AnnouncedConsumer {
	fn new(state: watch::Receiver<BTreeSet<Path>>, prefix: Path) -> Self {
		Self {
			state,
			active: BTreeSet::new(),
			prefix,
		}
	}

	/// Returns the next update.
	pub async fn next(&mut self) -> Option<Announced> {
		// NOTE: This just checks if the producer has been dropped.
		// We're not actually using the `changed()` state properly.
		while self.state.has_changed().is_ok() {
			while let Some(removed) = self.active.difference(&self.state.borrow()).next().cloned() {
				self.active.remove(&removed);
				if removed.has_prefix(&self.prefix) {
					return Some(Announced::Ended(removed));
				}
			}

			while let Some(added) = self.state.borrow().difference(&self.active).next().cloned() {
				self.active.insert(added.clone());
				if added.has_prefix(&self.prefix) {
					return Some(Announced::Active(added));
				}
			}

			if self.state.changed().await.is_err() {
				break;
			}
		}

		while let Some(removed) = self.active.pop_first() {
			if removed.has_prefix(&self.prefix) {
				return Some(Announced::Ended(removed));
			}
		}

		None
	}

	/// Returns the prefix in use.
	pub fn prefix(&self) -> &Path {
		&self.prefix
	}
}

#[cfg(test)]
mod test {
	use std::collections::HashSet;

	use futures::FutureExt;

	use super::*;

	#[test]
	fn simple() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let path = Path::default().push("a").push("b");

		assert!(!producer.is_active(&path));
		assert!(producer.announce(path.clone()));
		assert!(producer.is_active(&path));

		let announced = consumer.next().now_or_never().unwrap().unwrap();
		assert!(matches!(announced, Announced::Active(active) if active == path));

		assert!(producer.unannounce(&path));
		assert!(!producer.is_active(&path));

		let announced = consumer.next().now_or_never().unwrap().unwrap();
		assert!(matches!(announced, Announced::Ended(active) if active == path));

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn multi() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let path1 = Path::default().push("a").push("b");
		let path2 = Path::default().push("a").push("c");
		let path3 = Path::default().push("d").push("e");

		let mut paths: HashSet<Path> = HashSet::from_iter([path1, path2, path3]);
		for path in &paths {
			assert!(!producer.is_active(path));
			assert!(producer.announce(path.clone()));
			assert!(producer.is_active(&path));
		}

		// Make sure we get all of the paths only once.
		while !paths.is_empty() {
			let res = consumer.next().now_or_never().unwrap().unwrap();
			assert!(paths.remove(&res.active().unwrap()));
		}

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn late() {
		let mut producer = AnnouncedProducer::new();

		let path1 = Path::default().push("a").push("b");
		let path2 = Path::default().push("a").push("c");
		let path3 = Path::default().push("d").push("e");

		let mut paths: HashSet<Path> = HashSet::from_iter([path1, path2, path3]);
		for path in &paths {
			assert!(!producer.is_active(path));
			assert!(producer.announce(path.clone()));
			assert!(producer.is_active(&path));
		}

		// Subscribe after announcing.
		let mut consumer = producer.subscribe();

		// Make sure we get all of the paths only once.
		while !paths.is_empty() {
			let res = consumer.next().now_or_never().unwrap().unwrap();
			assert!(paths.remove(&res.active().unwrap()));
		}

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn prefix() {
		let mut producer = AnnouncedProducer::new();
		let prefix = Path::default().push("a");
		let mut consumer = producer.subscribe_prefix(prefix);

		let path1 = Path::default().push("a").push("b");
		let path2 = Path::default().push("a").push("c");
		let path3 = Path::default().push("d").push("e");

		assert!(producer.announce(path1.clone()));
		assert!(producer.announce(path2.clone()));
		assert!(producer.announce(path3.clone()));

		let mut expected: HashSet<Path> = HashSet::from_iter([path1, path2]);

		while !expected.is_empty() {
			let res = consumer.next().now_or_never().unwrap().unwrap();
			assert!(expected.remove(&res.active().unwrap()));
		}

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn prefix_unannounce() {
		let mut producer = AnnouncedProducer::new();
		let prefix = Path::default().push("a");
		let mut consumer = producer.subscribe_prefix(prefix);

		let path1 = Path::default().push("a").push("b");
		let path2 = Path::default().push("a").push("c");
		let path3 = Path::default().push("d").push("e");

		assert!(producer.announce(path1.clone()));
		assert!(producer.announce(path2.clone()));
		assert!(producer.announce(path3.clone()));

		let res = match consumer.next().now_or_never().unwrap().unwrap() {
			Announced::Active(active) if active == path1 || active == path2 => active,
			res => panic!("unexpected announcement: {:?}", res),
		};

		assert!(producer.unannounce(&path1));
		assert!(producer.unannounce(&path2));
		assert!(producer.unannounce(&path3));

		match consumer.next().now_or_never().unwrap().unwrap() {
			Announced::Ended(ended) if ended == res => ended,
			res => panic!("unexpected announcement: {:?}", res),
		};

		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn flicker() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let path = Path::default().push("a").push("b");

		assert!(!producer.is_active(&path));
		assert!(producer.announce(path.clone()));
		assert!(producer.is_active(&path));
		assert!(producer.unannounce(&path));
		assert!(!producer.is_active(&path));

		// We missed it.
		assert_eq!(consumer.next().now_or_never(), None);
	}

	#[test]
	fn dropped() {
		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let path1 = Path::default().push("a").push("b");
		let path2 = Path::default().push("a").push("c");
		let path3 = Path::default().push("d").push("e");

		producer.announce(path1.clone());
		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active(path1.clone()))
		);
		producer.announce(path2.clone());
		assert_eq!(
			consumer.next().now_or_never().unwrap(),
			Some(Announced::Active(path2.clone()))
		);

		// Don't consume path3 before dropping.
		producer.announce(path3);
		drop(producer);

		let res = match consumer.next().now_or_never().unwrap().unwrap() {
			Announced::Ended(ended) if ended == path1 || ended == path2 => ended,
			res => panic!("unexpected announcement: {:?}", res),
		};

		match consumer.next().now_or_never().unwrap().unwrap() {
			Announced::Ended(res1) if res1 == res => panic!("duplicate announcement: {:?}", res1),
			Announced::Ended(ended) if ended == path1 || ended == path2 => ended,
			res => panic!("unexpected announcement: {:?}", res),
		};

		// Since the producer is dropped, we immediately return None.
		assert_eq!(consumer.next().now_or_never().unwrap(), None);
	}

	#[tokio::test]
	async fn wakeup() {
		tokio::time::pause();

		let mut producer = AnnouncedProducer::new();
		let mut consumer = producer.subscribe();

		let path1 = Path::default().push("a").push("b");
		let path2 = Path::default().push("a").push("c");

		let p1 = path1.clone();
		let p2 = path2.clone();

		tokio::spawn(async move {
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce(p1.clone());
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.announce(p2);
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			producer.unannounce(&p1);
			tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
			// Don't actually unannounce p2, just drop.
			drop(producer);
		});

		let res = match consumer.next().await.unwrap() {
			Announced::Active(active) if active == path1 || active == path2 => active,
			res => panic!("unexpected announcement: {:?}", res),
		};

		match consumer.next().await.unwrap() {
			Announced::Active(dup) if dup == res => panic!("duplicate announcement: {:?}", dup),
			Announced::Active(active) if active == path1 || active == path2 => active,
			res => panic!("unexpected announcement: {:?}", res),
		};

		assert_eq!(consumer.next().await.unwrap(), Announced::Ended(path1));
		assert_eq!(consumer.next().await.unwrap(), Announced::Ended(path2));
		assert_eq!(consumer.next().await, None);
	}
}
