use std::sync::{Arc, Mutex, MutexGuard};

// Wrapper that makes working with Arc<Mutex<T>> easier.
pub struct Shared<T> {
	guard: Arc<Mutex<T>>,
}

impl<T> Shared<T> {
	pub fn new(data: T) -> Self {
		Self {
			guard: Arc::new(Mutex::new(data)),
		}
	}

	pub fn lock(&mut self) -> MutexGuard<T> {
		self.guard.lock().unwrap()
	}
}

impl<T> Clone for Shared<T> {
	fn clone(&self) -> Self {
		Self {
			guard: self.guard.clone(),
		}
	}
}
