use std::{
	fmt,
	future::Future,
	ops::{Deref, DerefMut},
	pin::Pin,
	sync::{Arc, Mutex, MutexGuard},
	task,
};

struct State<T> {
	value: T,
	wakers: Vec<task::Waker>,
	epoch: usize,
}

impl<T> State<T> {
	pub fn new(value: T) -> Self {
		Self {
			value,
			wakers: Vec::new(),
			epoch: 0,
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

impl<T: Default> Default for State<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: fmt::Debug> fmt::Debug for State<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.value.fmt(f)
	}
}

pub struct Watch<T> {
	state: Arc<Mutex<State<T>>>,
}

impl<T> Watch<T> {
	pub fn new(initial: T) -> Self {
		let state = Arc::new(Mutex::new(State::new(initial)));
		Self { state }
	}

	pub fn lock(&self) -> WatchRef<T> {
		WatchRef {
			state: self.state.clone(),
			lock: self.state.lock().unwrap(),
		}
	}

	pub fn lock_mut(&self) -> WatchMut<T> {
		WatchMut {
			lock: self.state.lock().unwrap(),
		}
	}
}

impl<T> Clone for Watch<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
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
			Ok(lock) => lock.value.fmt(f),
			Err(_) => write!(f, "<locked>"),
		}
	}
}

pub struct WatchRef<'a, T> {
	state: Arc<Mutex<State<T>>>,
	lock: MutexGuard<'a, State<T>>,
}

impl<'a, T> WatchRef<'a, T> {
	// Release the lock and wait for a notification when next updated.
	pub fn changed(self) -> WatchChanged<T> {
		WatchChanged {
			state: self.state,
			epoch: self.lock.epoch,
		}
	}

	// Release the lock and provide a context to wake when next updated.
	pub fn waker(mut self, cx: &mut task::Context<'_>) {
		self.lock.register(cx.waker());
	}

	// Upgrade to a mutable references that automatically calls notify on drop.
	pub fn into_mut(self) -> WatchMut<'a, T> {
		WatchMut { lock: self.lock }
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
	lock: MutexGuard<'a, State<T>>,
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
	state: Arc<Mutex<State<T>>>,
	epoch: usize,
}

impl<T> Future for WatchChanged<T> {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
		// TODO is there an API we can make that doesn't drop this lock?
		let mut state = self.state.lock().unwrap();

		if state.epoch > self.epoch {
			task::Poll::Ready(())
		} else {
			state.register(cx.waker());
			task::Poll::Pending
		}
	}
}
