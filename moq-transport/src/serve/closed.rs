// TODO actually use something like this to avoid the duplication

struct State {
	closed: Watch<Result<(), ServeError>>,
}

impl State {
	pub fn new() -> Self {
		let state = Watch::new(Ok(()));
		Self { state }
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		let closed = self.closed.lock();
		if let Err(err) = &closed {
			return Err(closed.clone());
		}

		*closed.lock_mut() = Err(err);
		Ok(())
	}

	pub fn closed(&self) -> Result<(), ServeError> {
		if let Err(err) = &self.closed.lock() {
			return Err(closed.clone());
		}
	}
}

/// A helper that that allows either a writer or reader to close the stream.
#[derive(Clone)]
pub(super) struct Closed {
	state: Arc<State>,
}

impl Closed {
	pub fn new() -> (Self, Self) {
		let state = Arc::new(State::new());
		(Closed { state }, Closed { state })
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.close(err)
	}

	pub fn closed(&self) -> Result<(), ServeError> {
		self.state.closed()
	}

	pub async fn wait(&self) -> ServeError {
		loop {
			let notify = {
				let state = self.state.closed.lock();
				if let Err(err) = state.state.closed {
					return err.clone();
				}

				state.changed()
			};

			notify.await;
		}
	}
}
