use std::{
	fmt,
	future::Future,
	ops::{Deref, DerefMut},
	pin::Pin,
	sync::{Arc, Mutex, MutexGuard, Weak},
	task,
};

struct StateInner<T> {
	value: T,
	wakers: Vec<task::Waker>,
	epoch: usize,
	dropped: Option<()>,
}

impl<T> StateInner<T> {
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

impl<T: Default> Default for StateInner<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: fmt::Debug> fmt::Debug for StateInner<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.value.fmt(f)
	}
}

pub struct State<T> {
	state: Arc<Mutex<StateInner<T>>>,
	drop: Arc<StateDrop<T>>,
}

impl<T> State<T> {
	pub fn new(initial: T) -> Self {
		let state = Arc::new(Mutex::new(StateInner::new(initial)));

		Self {
			state: state.clone(),
			drop: Arc::new(StateDrop { state }),
		}
	}

	pub fn lock(&self) -> StateRef<T> {
		StateRef {
			state: self.state.clone(),
			drop: self.drop.clone(),
			lock: self.state.lock().unwrap(),
		}
	}

	pub fn lock_mut(&self) -> Option<StateMut<T>> {
		let lock = self.state.lock().unwrap();
		lock.dropped?;
		Some(StateMut {
			lock,
			_drop: self.drop.clone(),
		})
	}

	pub fn downgrade(&self) -> StateWeak<T> {
		StateWeak {
			state: Arc::downgrade(&self.state),
			drop: Arc::downgrade(&self.drop),
		}
	}

	pub fn split(self) -> (Self, Self) {
		let state = self.state.clone();
		(
			self, // important that we don't make a new drop here
			Self {
				state: state.clone(),
				drop: Arc::new(StateDrop { state }),
			},
		)
	}
}

impl<T> Clone for State<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			drop: self.drop.clone(),
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
		match self.state.try_lock() {
			Ok(lock) => lock.value.fmt(f),
			Err(_) => write!(f, "<locked>"),
		}
	}
}

pub struct StateRef<'a, T> {
	state: Arc<Mutex<StateInner<T>>>,
	lock: MutexGuard<'a, StateInner<T>>,
	drop: Arc<StateDrop<T>>,
}

impl<'a, T> StateRef<'a, T> {
	// Release the lock and wait for a notification when next updated.
	pub fn modified(self) -> Option<StateChanged<T>> {
		self.lock.dropped?;

		Some(StateChanged {
			state: self.state,
			epoch: self.lock.epoch,
		})
	}

	// Upgrade to a mutable references that automatically calls notify on drop.
	pub fn into_mut(self) -> Option<StateMut<'a, T>> {
		self.lock.dropped?;
		Some(StateMut {
			lock: self.lock,
			_drop: self.drop,
		})
	}
}

impl<'a, T> Deref for StateRef<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.lock.value
	}
}

impl<'a, T: fmt::Debug> fmt::Debug for StateRef<'a, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.lock.fmt(f)
	}
}

pub struct StateMut<'a, T> {
	lock: MutexGuard<'a, StateInner<T>>,
	_drop: Arc<StateDrop<T>>,
}

impl<'a, T> Deref for StateMut<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.lock.value
	}
}

impl<'a, T> DerefMut for StateMut<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.lock.value
	}
}

impl<'a, T> Drop for StateMut<'a, T> {
	fn drop(&mut self) {
		self.lock.notify();
	}
}

impl<'a, T: fmt::Debug> fmt::Debug for StateMut<'a, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.lock.fmt(f)
	}
}

pub struct StateChanged<T> {
	state: Arc<Mutex<StateInner<T>>>,
	epoch: usize,
}

impl<T> Future for StateChanged<T> {
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

pub struct StateWeak<T> {
	state: Weak<Mutex<StateInner<T>>>,
	drop: Weak<StateDrop<T>>,
}

impl<T> StateWeak<T> {
	pub fn upgrade(&self) -> Option<State<T>> {
		if let (Some(state), Some(drop)) = (self.state.upgrade(), self.drop.upgrade()) {
			Some(State { state, drop })
		} else {
			None
		}
	}
}

impl<T> Clone for StateWeak<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			drop: self.drop.clone(),
		}
	}
}

struct StateDrop<T> {
	state: Arc<Mutex<StateInner<T>>>,
}

impl<T> Drop for StateDrop<T> {
	fn drop(&mut self) {
		let mut state = self.state.lock().unwrap();
		state.dropped = None;
		state.notify();
	}
}
