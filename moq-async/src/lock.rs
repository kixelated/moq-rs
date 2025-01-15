use std::fmt;
use std::ops::{Deref, DerefMut};

// It's a cosmetic wrapper around Arc<Mutex<T>> on native platforms.
// On WASM, it uses Rc<RefCell<T>> instead.
pub struct Lock<T> {
	#[cfg(any(not(target_arch = "wasm32"), target_os = "wasi"))]
	inner: std::sync::Arc<std::sync::Mutex<T>>,

	#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
	inner: std::rc::Rc<std::cell::RefCell<T>>,
}

#[cfg(any(not(target_arch = "wasm32"), target_os = "wasi"))]
impl<T> Lock<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: std::sync::Arc::new(std::sync::Mutex::new(value)),
		}
	}

	pub fn lock(&self) -> LockGuard<T> {
		LockGuard {
			inner: self.inner.lock().unwrap(),
		}
	}
}

#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
impl<T> Lock<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: std::rc::Rc::new(std::cell::RefCell::new(value)),
		}
	}

	pub fn lock(&self) -> LockGuard<T> {
		LockGuard {
			inner: self.inner.borrow_mut(),
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
	#[cfg(any(not(target_arch = "wasm32"), target_os = "wasi"))]
	inner: std::sync::MutexGuard<'a, T>,

	#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
	inner: std::cell::RefMut<'a, T>,
}

impl<T> Deref for LockGuard<'_, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T> DerefMut for LockGuard<'_, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl<T: fmt::Debug> fmt::Debug for LockGuard<'_, T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.inner.fmt(f)
	}
}
