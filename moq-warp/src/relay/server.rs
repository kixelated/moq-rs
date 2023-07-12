use super::broker;

use std::{net, path};

pub struct ServerConfig {
	pub addr: net::SocketAddr,
	pub cert: path::PathBuf,
	pub key: path::PathBuf,

	pub broker: broker::Broadcasts,
}
