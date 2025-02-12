pub struct Stream {
	buffer: Vec<u8>,
	skip: usize,
}

impl Stream {
	pub fn recv(&mut self, mut buf: &[u8]) -> Result<(), Error> {
		let mut buffer = self.buffer.take().unwrap();

		loop {
			// Try to read from the buffer then the input
			let slices = buffer.as_slices();
			let mut chain = slices.0.chain(slices.1).chain(buf);

			match self.recv_loop(&mut chain) {
				Ok(()) => {
					let n = chain.remaining() - buffer.len() - buf.len();
					if n > buffer.len() {
						buf = &buf[buffer.len() - n..];
						buffer.clear();
					} else {
						buffer.advance(n);
					}
				}
				Err(Error::Coding(DecodeError::Short)) => {
					buffer.extend(buf);
					self.buffer = Some(buffer);
					return Ok(());
				}
				Err(err) => return Err(err),
			}
		}
	}
}
