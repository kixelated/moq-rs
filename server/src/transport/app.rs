use std::time;

use quiche::h3::webtransport;

pub trait App: Default {
    fn poll(
        &mut self,
        conn: &mut quiche::Connection,
        session: &mut webtransport::ServerSession,
    ) -> anyhow::Result<()>;
    fn timeout(&self) -> Option<time::Duration>;
}
