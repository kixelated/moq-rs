use std::collections::HashMap;

use crate::{AnnouncedConsumer, Error, Filter, Track, TrackConsumer};

use moq_async::{spawn, Lock};
use moq_transfork_proto::SubscribeRequest;
use tokio::sync::mpsc;

/// A MoqTransfork session, used to publish and/or subscribe to broadcasts.
///
/// A publisher will [Self::publish] tracks, or alternatively [Self::announce] and [Self::route] arbitrary paths.
/// A subscriber will [Self::subscribe] to tracks, or alternatively use [Self::announced] to discover arbitrary paths.
pub struct Connection {
	proto: Lock<moq_transfork_proto::Connection>,
	publisher: mpsc::Receiver<moq_transfork_proto::PublisherEvent>,
}

impl Connection {
	fn new(web: web_transport::Session, proto: moq_transfork_proto::Connection) -> Self {
		let proto = Lock::new(proto);
		let publisher = mpsc::channel(100);

		let backend = Backend {
			web,
			proto: proto.clone(),
			publisher: publisher.0,
		};

		spawn(async move {
			backend.run().await.expect("connection error");
		});

		Self {
			proto,
			publisher: publisher.1,
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
	pub fn publish(&mut self, track: TrackConsumer) {}

	/// Subscribe to a track and start receiving data over the network.
	pub fn subscribe(&self, track: Track) -> TrackConsumer {
		let mut proto = self.proto.lock();
		let mut subscriber = proto.subscriber();
		let mut subscribes = subscriber.subscribes();

		let subscribe = subscribes.create(SubscribeRequest {
			path: track.path,
			priority: track.priority,
			order: track.order,
		});

		let (producer, consumer) = track.produce();

		consumer
	}

	/// Discover any tracks published by the remote matching a (wildcard) filter.
	pub fn announced(&self, filter: Filter) -> AnnouncedConsumer {
		let proto = self.proto.lock();
		let subscriber = proto.subscriber();
		subscriber.announced(filter)
	}
}

struct Backend {
	web: web_transport::Session,
	proto: Lock<moq_transfork_proto::Connection>,
	publisher: mpsc::Sender<moq_transfork_proto::PublisherEvent>,
}

impl Backend {
	async fn run(mut self) -> Result<(), Error> {
		let mut recv_streams = HashMap::new();
		let mut send_streams = HashMap::new();
		let mut stream_id = moq_transfork_proto::StreamId::default();

		let mut buffer = Vec::new();

		loop {
			let proto = self.proto.lock();
			let mut streams = proto.streams();

			while let Some(event) = streams.poll() {
				match event {
					moq_transfork_proto::StreamEvent::Open(dir) => match dir {
						moq_transfork_proto::StreamDirection::Uni => {
							let mut send_stream = self.web.open_uni().await?;
							stream_id.increment();

							let mut stream = streams.open(dir, stream_id.into());
							stream.encode(&mut buffer);
							send_stream.write(&buffer).await?;
							buffer.clear();

							send_streams.insert(stream_id, send_stream);
						}
						moq_transfork_proto::StreamDirection::Bi => {
							let (mut send_stream, recv_stream) = self.web.open_bi().await?;
							stream_id.increment();

							let mut stream = streams.open(dir, stream_id.into());
							stream.encode(&mut buffer);
							send_stream.write(&buffer).await?;
							buffer.clear();

							recv_streams.insert(stream_id, recv_stream);
							send_streams.insert(stream_id, send_stream);
						}
					},
					moq_transfork_proto::StreamEvent::Encode(id) => {
						let send = send_streams.get_mut(&id).unwrap();
						let mut stream = streams.get(id).unwrap();

						stream.encode(&mut buffer);
						send.write(&buffer).await?;
						buffer.clear();
					}
				}
			}

			let mut publisher = proto.publisher();
			while let Some(event) = publisher.poll() {
				self.publisher.send(event).await?;
			}

			// TODO loop until all events are processed
			tokio::select! {
				Some(announce) = self.subscriber_announces.recv() => {
				}
				Some(subscribe) = self.subscriber_subscribes.recv() => {
				}
			}
		}
	}
}
