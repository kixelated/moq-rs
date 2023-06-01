use super::Session;
use crate::{media, transport};

use std::time;

use quiche::h3::webtransport;

pub struct Server {
	// The media source
	media: media::Source,
}

impl Server {
	// Create a new server
	pub fn new(media: media::Source) -> Self {
		Self { media }
	}
}

impl transport::app::Server for Server {
	type Session = Session;

	fn accept(&mut self, _req: webtransport::ConnectRequest) -> anyhow::Result<Session> {
		let subscription = self.media.subscribe();

		let session = Session::new(subscription);
		Ok(session)
	}

	// Called periodically based on the timeout returned.
	fn poll(&mut self) -> anyhow::Result<()> {
		self.media.poll().into()
	}

	fn timeout(&self) -> Option<time::Duration> {
		self.media.timeout()
	}
}
