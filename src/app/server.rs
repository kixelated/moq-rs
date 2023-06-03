use super::connection::Connection;
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
	type Connection = Connection;

	fn accept(&mut self, mut conn: quiche::Connection) -> anyhow::Result<Self::Connection> {
		let session = webtransport::ServerSession::with_transport(&mut conn)?;

		let subscription = self.media.subscribe();

		let session = Connection::new(conn, session, subscription);
		Ok(session)
	}

	// Called periodically based on the timeout returned.
	fn poll(&mut self) -> anyhow::Result<Option<time::Duration>> {
		self.media.poll()
	}
}
