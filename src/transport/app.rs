use std::time;

use quiche::h3::webtransport;

pub trait Server {
	type Session: Session;

	// TODO take ownership of a connection
	fn accept(&mut self, req: webtransport::ConnectRequest) -> anyhow::Result<Self::Session>;

	// TODO combine these two
	fn poll(&mut self) -> anyhow::Result<()>;
	fn timeout(&self) -> Option<time::Duration>;
}

pub trait Session {
	// TODO combine these two
	fn poll(&mut self, conn: &mut quiche::Connection, session: &mut webtransport::ServerSession) -> anyhow::Result<()>;
	fn timeout(&self) -> Option<time::Duration>;
}
