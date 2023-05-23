use quiche;
use quiche::h3::webtransport;

use std::collections::hash_map as hmap;

pub type Id = quiche::ConnectionId<'static>;

use super::app;

pub type Map<T> = hmap::HashMap<Id, Connection<T>>;
pub struct Connection<T: app::App> {
	pub quiche: quiche::Connection,
	pub session: Option<webtransport::ServerSession>,
	pub app: T,
}
