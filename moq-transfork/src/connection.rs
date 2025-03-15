use std::sync::{Arc, Mutex};

use crate::{AnnouncedConsumer, Error, Filter, RouterConsumer, Track, TrackConsumer};
use moq_transfork_proto::message;

use moq_async::{spawn, Close, OrClose};

/// A MoqTransfork session, used to publish and/or subscribe to broadcasts.
///
/// A publisher will [Self::publish] tracks, or alternatively [Self::announce] and [Self::route] arbitrary paths.
/// A subscriber will [Self::subscribe] to tracks, or alternatively use [Self::announced] to discover arbitrary paths.
#[derive(Clone)]
pub struct Connection {
	webtransport: web_transport::Session,
}

impl Connection {
	fn new(session: web_transport::Session, proto: moq_transfork_proto::Connection) -> Self {
		spawn(Self::run(proto));
		Self { webtransport: session }
	}

	async fn run(proto: moq_transfork_proto::Connection) {
		loop {
			proto.poll().await;
		}
	}

	/// Perform the MoQ handshake as a client.
	pub async fn connect<T: Into<web_transport::Session>>(session: T) -> Result<Self, Error> {
		let session = session.into();
		let proto = moq_transfork_proto::Connection::client();
		Ok(Self::new(session, proto))
	}

	/// Perform the MoQ handshake as a server
	pub async fn accept<T: Into<web_transport::Session>>(session: T) -> Result<Self, Error> {
		let session = session.into();
		let proto = moq_transfork_proto::Connection::server();
		Ok(Self::new(session, proto))
	}

	/// Publish a track, automatically announcing and serving it.
	pub fn publish(&mut self, track: TrackConsumer) -> Result<(), Error> {}

	/// Optionally announce the provided tracks.
	///
	/// This is advanced functionality if you wish to perform dynamic track generation in conjunction with [Self::route].
	/// [AnnouncedConsumer] will automatically unannounce if the [crate::AnnouncedProducer] is dropped.
	pub fn announce(&mut self, announced: AnnouncedConsumer) {}

	/// Optionally route unknown paths.
	///
	/// This is advanced functionality if you wish to perform dynamic track generation in conjunction with [Self::announce].
	pub fn route(&mut self, router: RouterConsumer) {}

	/// Subscribe to a track and start receiving data over the network.
	pub fn subscribe(&self, track: Track) -> TrackConsumer {}

	/// Discover any tracks published by the remote matching a (wildcard) filter.
	pub fn announced(&self, filter: Filter) -> AnnouncedConsumer {}

	/// Close the underlying WebTransport session.
	pub fn close(mut self, err: Error) {
		self.webtransport.close(1, &err.to_string());
	}

	/// Block until the WebTransport session is closed.
	pub async fn closed(&self) -> Error {
		self.webtransport.closed().await.into()
	}
}

impl PartialEq for Connection {
	fn eq(&self, other: &Self) -> bool {
		self.webtransport == other.webtransport
	}
}

impl Eq for Connection {}
