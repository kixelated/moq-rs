use crate::MoqError;

pub(crate) trait Close {
	fn close(&mut self, err: MoqError);
}

pub(crate) trait OrClose<S: Close, V> {
	fn or_close(self, stream: &mut S) -> Result<V, MoqError>;
}

impl<S: Close, V> OrClose<S, V> for Result<V, MoqError> {
	fn or_close(self, stream: &mut S) -> Result<V, MoqError> {
		match self {
			Ok(v) => Ok(v),
			Err(err) => {
				stream.close(err.clone());
				Err(err)
			}
		}
	}
}
