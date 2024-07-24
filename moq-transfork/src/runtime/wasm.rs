use std::{cell, future, ops, rc};

pub fn spawn<F: future::Future<Output = ()> + 'static>(f: F) {
	wasm_bindgen_futures::spawn_local(f);
}

pub struct Ref<T> {
	inner: rc::Rc<T>,
}

impl<T> Ref<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: rc::Rc::new(value),
		}
	}

	pub fn downgrade(&self) -> RefWeak<T> {
		RefWeak {
			inner: rc::Rc::downgrade(&self.inner),
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
	inner: rc::Weak<T>,
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
	inner: Ref<cell::RefCell<T>>,
}

impl<T> Lock<T> {
	pub fn new(value: T) -> Self {
		Self {
			inner: Ref::new(cell::RefCell::new(value)),
		}
	}

	pub fn lock(&self) -> LockGuard<T> {
		LockGuard {
			inner: self.inner.borrow_mut(),
		}
	}

	pub fn try_lock(&self) -> Option<LockGuard<T>> {
		self.inner.try_borrow_mut().ok().map(|inner| LockGuard { inner })
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
	inner: cell::RefMut<'a, T>,
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
	inner: RefWeak<cell::RefCell<T>>,
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
	inner: Lock<u64>,
}

impl Counter {
	pub fn new(value: u64) -> Self {
		Self {
			inner: Lock::new(value),
		}
	}

	pub fn add(&self, value: u64) -> u64 {
		let lock = self.inner.lock();
		let prev = *lock;
		*lock += value;
		prev
	}

	pub fn sub(&self, value: u64) -> u64 {
		let lock = self.inner.lock();
		let prev = *lock;
		*lock -= value;
		prev
	}

	pub fn get(&self) -> u64 {
		let lock = self.inner.lock();
		*lock
	}

	pub fn set(&self, value: u64) {
		let lock = self.inner.lock();
		*lock = value;
	}
}
