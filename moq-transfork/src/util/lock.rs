use std::{fmt, ops, sync};

// It's just a cosmetic wrapper around Arc/Mutex
pub struct Lock<T> {
	inner: sync::Arc<sync::Mutex<T>>,
}

impl<T> Lock<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: sync::Arc::new(sync::Mutex::new(value)),
		}
	}

	pub fn lock(&self) -> LockGuard<T> {
		LockGuard {
			inner: self.inner.lock().unwrap(),
		}
	}
}

impl<T: Default> Default for Lock<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T> Clone for Lock<T> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

pub struct LockGuard<'a, T> {
	inner: sync::MutexGuard<'a, T>,
}

impl<T> ops::Deref for LockGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T> ops::DerefMut for LockGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl<T: fmt::Debug> fmt::Debug for LockGuard<'_, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.inner.fmt(f)
	}
}
