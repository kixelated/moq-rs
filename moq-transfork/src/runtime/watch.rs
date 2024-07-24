use std::{
	fmt,
	future::Future,
	ops::{Deref, DerefMut},
	pin::Pin,
	sync::{Arc, Weak},
	task,
};

use super::{Lock, LockGuard, LockWeak};

struct WatchState<T> {
	value: T,
	wakers: Vec<task::Waker>,
	epoch: usize,
	dropped: Option<()>,
}

impl<T> WatchState<T> {
	pub fn new(value: T) -> Self {
		Self {
			value,
			wakers: Vec::new(),
			epoch: 0,
			dropped: Some(()),
		}
	}

	pub fn register(&mut self, waker: &task::Waker) {
		self.wakers.retain(|existing| !existing.will_wake(waker));
		self.wakers.push(waker.clone());
	}

	pub fn notify(&mut self) {
		self.epoch += 1;
		for waker in self.wakers.drain(..) {
			waker.wake();
		}
	}
}

impl<T: Default> Default for WatchState<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: fmt::Debug> fmt::Debug for WatchState<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.value.fmt(f)
	}
}

pub struct Watch<T> {
	state: Lock<WatchState<T>>,
	drop: Arc<WatchDrop<T>>,
}

impl<T> Watch<T> {
	pub fn new(initial: T) -> Self {
		let state = Lock::new(WatchState::new(initial));

		Self {
			state: state.clone(),
			drop: Arc::new(WatchDrop { state }),
		}
	}

	pub fn lock(&self) -> WatchRef<T> {
		WatchRef {
			state: self.state.clone(),
			drop: self.drop.clone(),
			lock: self.state.lock(),
		}
	}

	pub fn lock_mut(&self) -> Option<WatchMut<T>> {
		let lock = self.state.lock();
		lock.dropped?;
		Some(WatchMut {
			lock,
			_drop: self.drop.clone(),
		})
	}

	pub fn downgrade(&self) -> WatchWeak<T> {
		WatchWeak {
			state: self.state.downgrade(),
			drop: Arc::downgrade(&self.drop),
		}
	}

	pub fn split(&self) -> Self {
		let state = self.state.clone();
		Self {
			state: state.clone(),
			drop: Arc::new(WatchDrop { state }),
		}
	}
}

impl<T> Clone for Watch<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			drop: self.drop.clone(),
		}
	}
}

impl<T: Default> Default for Watch<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: fmt::Debug> fmt::Debug for Watch<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.state.try_lock() {
			Some(lock) => lock.value.fmt(f),
			None => write!(f, "<locked>"),
		}
	}
}

pub struct WatchRef<'a, T> {
	state: Lock<WatchState<T>>,
	lock: LockGuard<'a, WatchState<T>>,
	drop: Arc<WatchDrop<T>>,
}

impl<'a, T> WatchRef<'a, T> {
	// Release the lock and wait for a notification when next updated.
	pub fn changed(self) -> Option<WatchChanged<T>> {
		self.lock.dropped?;

		Some(WatchChanged {
			state: self.state,
			epoch: self.lock.epoch,
		})
	}

	// Upgrade to a mutable references that automatically calls notify on drop.
	pub fn into_mut(self) -> Option<WatchMut<'a, T>> {
		self.lock.dropped?;
		Some(WatchMut {
			lock: self.lock,
			_drop: self.drop,
		})
	}
}

impl<'a, T> Deref for WatchRef<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.lock.value
	}
}

impl<'a, T: fmt::Debug> fmt::Debug for WatchRef<'a, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.lock.fmt(f)
	}
}

pub struct WatchMut<'a, T> {
	lock: LockGuard<'a, WatchState<T>>,
	_drop: Arc<WatchDrop<T>>,
}

impl<'a, T> Deref for WatchMut<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.lock.value
	}
}

impl<'a, T> DerefMut for WatchMut<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.lock.value
	}
}

impl<'a, T> Drop for WatchMut<'a, T> {
	fn drop(&mut self) {
		self.lock.notify();
	}
}

impl<'a, T: fmt::Debug> fmt::Debug for WatchMut<'a, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.lock.fmt(f)
	}
}

pub struct WatchChanged<T> {
	state: Lock<WatchState<T>>,
	epoch: usize,
}

impl<T> Future for WatchChanged<T> {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
		// TODO is there an API we can make that doesn't drop this lock?
		let mut state = self.state.lock();

		if state.epoch > self.epoch {
			task::Poll::Ready(())
		} else {
			state.register(cx.waker());
			task::Poll::Pending
		}
	}
}

pub struct WatchWeak<T> {
	state: LockWeak<WatchState<T>>,
	drop: Weak<WatchDrop<T>>,
}

impl<T> WatchWeak<T> {
	pub fn upgrade(&self) -> Option<Watch<T>> {
		if let (Some(state), Some(drop)) = (self.state.upgrade(), self.drop.upgrade()) {
			Some(Watch { state, drop })
		} else {
			None
		}
	}
}

impl<T> Clone for WatchWeak<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			drop: self.drop.clone(),
		}
	}
}

struct WatchDrop<T> {
	state: Lock<WatchState<T>>,
}

impl<T> Drop for WatchDrop<T> {
	fn drop(&mut self) {
		let mut state = self.state.lock();
		state.dropped = None;
		state.notify();
	}
}
