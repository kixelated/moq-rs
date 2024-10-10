use crate::Error;

pub trait Close {
	fn close(&mut self, err: Error);
}

pub trait OrClose<S: Close, V> {
	fn or_close(self, stream: &mut S) -> Result<V, Error>;
}

impl<S: Close, V> OrClose<S, V> for Result<V, Error> {
	fn or_close(self, stream: &mut S) -> Result<V, Error> {
		match self {
			Ok(v) => Ok(v),
			Err(err) => {
				stream.close(err.clone());
				Err(err)
			}
		}
	}
}
