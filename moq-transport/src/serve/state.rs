use std::{fmt, ops, sync::Arc};

use crate::util::{Watch, WatchMut, WatchRef};

use super::ServeError;

pub struct State<T> {
	inner: T,
	closed: Result<(), ServeError>,
}

impl<T> State<T> {
	fn new(init: T) -> (StateWriter<T>, StateReader<T>) {
		let state = Watch::new(Self {
			inner: init,
			closed: Ok(()),
		});

		let writer = StateWriter::new(state.clone());
		let reader = StateReader::new(state);

		(writer, reader)
	}

	// Doesn't return an error since lock() will.
	pub fn close(&mut self, err: ServeError) {
		self.closed = Err(err);
	}

	// TODO make changed() return the error instead.
	pub fn closed(&self) -> Result<(), ServeError> {
		self.closed.clone()
	}
}

impl<T: Default> State<T> {
	pub fn default() -> (StateWriter<T>, StateReader<T>) {
		Self::new(T::default())
	}
}

impl<T> ops::Deref for State<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T> ops::DerefMut for State<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.inner
	}
}

impl<T: fmt::Debug> fmt::Debug for State<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		f.debug_struct("State")
			.field("inner", &self.inner)
			.field("closed", &self.closed)
			.finish()
	}
}

pub struct StateWriter<T> {
	state: Watch<State<T>>,
	_dropped: Arc<StateDropped<T>>,
}

impl<T> StateWriter<T> {
	fn new(state: Watch<State<T>>) -> Self {
		let _dropped = Arc::new(StateDropped::new(state.clone()));
		Self { state, _dropped }
	}

	pub fn lock(&self) -> Result<WatchRef<State<T>>, ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;
		Ok(state)
	}

	pub fn lock_mut(&mut self) -> Result<WatchMut<State<T>>, ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;
		Ok(state.into_mut())
	}
}

impl<T: fmt::Debug> fmt::Debug for StateWriter<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Debug::fmt(&self.state, f)
	}
}

impl<T> Clone for StateWriter<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			_dropped: self._dropped.clone(),
		}
	}
}

pub struct StateReader<T> {
	state: Watch<State<T>>,
	_dropped: Arc<StateDropped<T>>,
}

impl<T> StateReader<T> {
	fn new(state: Watch<State<T>>) -> Self {
		let _dropped = Arc::new(StateDropped::new(state.clone()));
		Self { state, _dropped }
	}

	pub fn lock(&self) -> WatchRef<State<T>> {
		self.state.lock()
	}
}

impl<T: fmt::Debug> fmt::Debug for StateReader<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		fmt::Debug::fmt(&self.state, f)
	}
}

impl<T> Clone for StateReader<T> {
	fn clone(&self) -> Self {
		Self {
			state: self.state.clone(),
			_dropped: self._dropped.clone(),
		}
	}
}

pub struct StateDropped<T> {
	state: Watch<State<T>>,
}

impl<T> StateDropped<T> {
	fn new(state: Watch<State<T>>) -> Self {
		Self { state }
	}
}

impl<T> Drop for StateDropped<T> {
	fn drop(&mut self) {
		let state = self.state.lock();
		if state.closed.is_ok() {
			state.into_mut().close(ServeError::Done);
		}
	}
}
