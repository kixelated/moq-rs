#[derive(Clone)]
pub struct Connection {
	web_transport: web_transport::Session,
	proto: watch::Sender<moq_transfork_proto::Connection>,
}

impl Connection {
	pub fn accept(web_transport: web_transport::Session) -> Self {
		Self::new(web_transport)
	}

	pub fn connect(web_transport: web_transport::Session) -> Self {
		Self::new(web_transport)
	}

	fn new(web_transport: web_transport::Session) -> Self {
		let proto = moq_transfork_proto::Connection::new();

		let this = Self { web_transport, proto };
		tokio::spawn(this.clone().run);

		this
	}

	async fn run(mut self) {
		let reader = self.proto.subscribe();
		let mut streams = HashMap::new();

		let buf = Vec::new();

		loop {
			let borrow = reader.borrow_and_update();
			while let Some(stream) = borrow.encode(&mut buf) {
				self.streams.entry(stream)
			}

			reader.changed().await;
		}
	}

	/// Publish a track, automatically announcing and serving it.
	pub fn publish(&mut self, track: TrackConsumer) -> Result<(), Error> {
		self.publisher.publish(track)
	}

	/// Discover any tracks published by the remote matching a prefix.
	pub fn announced(&self, prefix: message::Path) -> AnnouncedConsumer {}

	/// Subscribe to a track and start receiving data over the network.
	pub fn subscribe(&self, track: Track) -> TrackConsumer {
		self.subscriber.subscribe(track)
	}
}
