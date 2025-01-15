use std::error::Error;

pub trait Close<E: Error + Clone> {
	fn close(&mut self, err: E);
}

pub trait OrClose<S: Close<E>, V, E: Error + Clone> {
	fn or_close(self, stream: &mut S) -> Result<V, E>;
}

impl<S: Close<E>, V, E: Error + Clone> OrClose<S, V, E> for Result<V, E> {
	fn or_close(self, stream: &mut S) -> Result<V, E> {
		match self {
			Ok(v) => Ok(v),
			Err(err) => {
				stream.close(err.clone());
				Err(err)
			}
		}
	}
}
