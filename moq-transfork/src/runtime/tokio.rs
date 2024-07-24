use std::{
	future, ops,
	sync::{self, atomic},
};

pub fn spawn<F: future::Future<Output = ()> + Send + 'static>(f: F) {
	tokio::task::spawn(f);
}

pub struct Ref<T> {
	inner: sync::Arc<T>,
}

impl<T> Ref<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: sync::Arc::new(value),
		}
	}

	pub fn downgrade(&self) -> RefWeak<T> {
		RefWeak {
			inner: sync::Arc::downgrade(&self.inner),
		}
	}
}

impl<T> Clone for Ref<T> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

impl<T> ops::Deref for Ref<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T: Default> Default for Ref<T> {
	fn default() -> Self {
		Self::new(T::default())
	}
}

pub struct RefWeak<T> {
	inner: sync::Weak<T>,
}

impl<T> RefWeak<T> {
	pub fn upgrade(&self) -> Option<Ref<T>> {
		self.inner.upgrade().map(|inner| Ref { inner })
	}
}

impl<T> Clone for RefWeak<T> {
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
		}
	}
}

pub struct Lock<T> {
	inner: Ref<sync::Mutex<T>>,
}

impl<T> Lock<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: Ref::new(sync::Mutex::new(value)),
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
			inner: self.inner.downgrade(),
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
	inner: RefWeak<sync::Mutex<T>>,
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

#[derive(Clone, Default)]
pub struct Counter {
	inner: Ref<atomic::AtomicU64>,
}

impl Counter {
	pub fn new(value: u64) -> Self {
		Self {
			inner: Ref::new(atomic::AtomicU64::new(value)),
		}
	}

	pub fn add(&self, value: u64) -> u64 {
		self.inner.fetch_add(value, atomic::Ordering::Relaxed)
	}

	pub fn sub(&self, value: u64) -> u64 {
		self.inner.fetch_sub(value, atomic::Ordering::Relaxed)
	}

	pub fn get(&self) -> u64 {
		self.inner.load(atomic::Ordering::Relaxed)
	}

	pub fn set(&self, value: u64) {
		self.inner.store(value, atomic::Ordering::Relaxed)
	}
}
