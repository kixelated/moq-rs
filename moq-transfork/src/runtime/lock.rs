use std::{ops, sync};

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

	pub fn try_lock(&self) -> Option<LockGuard<T>> {
		self.inner.try_lock().ok().map(|inner| LockGuard { inner })
	}

	pub fn downgrade(&self) -> LockWeak<T> {
		LockWeak {
			inner: sync::Arc::downgrade(&self.inner),
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

impl<'a, T> ops::Deref for LockGuard<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<'a, T> ops::DerefMut for LockGuard<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

pub struct LockWeak<T> {
	inner: sync::Weak<sync::Mutex<T>>,
}

impl<T> LockWeak<T> {
	pub fn upgrade(&self) -> Option<Lock<T>> {
		self.inner.upgrade().map(|inner| Lock { inner })
	}
}

impl<T> Clone for LockWeak<T> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}
