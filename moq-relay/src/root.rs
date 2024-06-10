pub struct Root {
	publisher: Publisher,
	subscriber: Subscriber,
}

impl Root {
	pub fn new(publisher: Publisher, subscriber: Subscriber) -> Self {
		Self { url }
	}

	pub fn announce(&mut self, broadcast: &str) -> Result<Announce, ServeError> {
		let url = self.url.as_ref().ok_or(ServeError::NoUrl)?;
		let client = quic::Client::new(url.clone())?;

		Ok(Announce {
			client,
			broadcast: broadcast.to_string(),
		})
	}

	pub async fn run(mut self) -> Result<(), ServeError> {
		Ok(())
	}
}
