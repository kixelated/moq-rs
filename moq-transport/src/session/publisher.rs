use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use webtransport_quinn::{RecvStream, SendStream, Session};

use crate::{
	message,
	message::Message,
	model::{broadcast, segment, track},
	Error, VarInt,
};

use super::Control;

#[derive(Debug, Default)]
struct Announces {
	lookup: HashMap<String, broadcast::Subscriber>,
}

#[derive(Debug, Default)]
struct Subscribes {
	lookup: HashMap<VarInt, track::Publisher>,
}

/// Serves broadcasts over the network, automatically handling subscriptions and caching.
#[derive(Clone, Debug)]
pub struct Publisher {
	announces: Arc<Mutex<Announces>>,
	subscribes: Arc<Mutex<Subscribes>>,

	webtransport: Session,
	control: Control,
}

impl Publisher {
	pub fn new(webtransport: Session, control: (SendStream, RecvStream)) -> Self {
		let (send, recv) = control;
		let control = Control::new(send);

		let this = Self {
			webtransport,
			announces: Default::default(),
			subscribes: Default::default(),
			control,
		};

		tokio::spawn(this.clone().run(recv));

		this
	}

	/// Serve a broadcast and send an ANNOUNCE.
	pub async fn announce(&mut self, broadcast: broadcast::Subscriber) -> Result<(), Error> {
		let namespace = broadcast.name.to_string();

		{
			let mut announces = self.announces.lock().unwrap();
			match announces.lookup.entry(namespace.clone()) {
				hash_map::Entry::Vacant(entry) => entry.insert(broadcast),
				hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			};
		}

		let msg = message::Announce { namespace };
		self.control.send(msg).await?;

		// TODO send ANNOUNCE_RESET on broadcast.closed()

		Ok(())
	}

	// TODO Serve a broadcast without sending an ANNOUNCE.
	// fn serve(&mut self, broadcast: broadcast::Subscriber) -> Result<(), Error> {

	// TODO Wait until the next subscribe that doesn't route to an ANNOUNCE.
	// pub async fn subscribed(&mut self) -> Result<track::Producer, Error> {

	pub fn close(self, code: u32) {
		self.webtransport.close(code, b"")
	}

	// Internal methods

	async fn run(mut self, mut control: RecvStream) -> Result<(), Error> {
		loop {
			tokio::select! {
				_stream = self.webtransport.accept_uni() => {
					return Err(Error::Role(VarInt::ZERO));
				}
				// NOTE: this is not cancel safe, but it's fine since the other branch is a fatal error.
				msg = Message::decode(&mut control) => {
					let msg = msg.map_err(|_x| Error::Read)?;

					log::info!("message received: {:?}", msg);
					if let Err(err) = self.recv_message(&msg).await {
						log::warn!("message error: {:?} {:?}", err, msg);
					}
				}
			}
		}
	}

	async fn recv_message(&mut self, msg: &Message) -> Result<(), Error> {
		match msg {
			Message::AnnounceOk(msg) => self.recv_announce_ok(msg).await,
			Message::AnnounceStop(msg) => self.recv_announce_stop(msg).await,
			Message::Subscribe(msg) => self.recv_subscribe(msg).await,
			Message::SubscribeReset(msg) => self.recv_subscribe_reset(msg).await,
			_ => Err(Error::Role(msg.id())),
		}
	}

	async fn recv_announce_ok(&mut self, _msg: &message::AnnounceOk) -> Result<(), Error> {
		// TODO do something
		Ok(())
	}

	async fn recv_announce_stop(&mut self, msg: &message::AnnounceStop) -> Result<(), Error> {
		let _announce = self
			.announces
			.lock()
			.unwrap()
			.lookup
			.remove(&msg.namespace)
			.ok_or(Error::NotFound)?;

		// broadcast::Subscriber is now dropped, but we continue to serve any existing track::Subscribers.
		// broadcast::Publisher will get Error::Closed when all track::Subscribers have terminated.

		let msg = message::AnnounceReset {
			namespace: msg.namespace.clone(),
			code: msg.code,
			reason: msg.reason.clone(),
		};

		self.control.send(msg).await?;

		Ok(())
	}

	async fn recv_subscribe(&mut self, msg: &message::Subscribe) -> Result<(), Error> {
		// Make a new track that we're going to populate.
		let (producer, _subscriber) = track::new(&msg.name);

		// Make sure the subscription ID is unique.
		match self.subscribes.lock().unwrap().lookup.entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(entry) => entry.insert(producer.clone()), // So we have a handle to close the producer.
		};

		// Now that we have a unique subscription ID, start the subscribe or send an error.
		if let Err(err) = self.start_subscribe(msg.id, &msg.namespace, producer) {
			let msg = message::SubscribeReset {
				id: msg.id,
				code: err.code(),
				reason: err.reason().to_string(),
			};

			self.control.send(msg).await?;

			return Err(err);
		}

		self.control.send(message::SubscribeOk { id: msg.id }).await?;

		Ok(())
	}

	fn start_subscribe(&mut self, id: VarInt, broadcast: &str, mut track: track::Publisher) -> Result<(), Error> {
		log::info!("starting subscribe: broadcast={} track={}", broadcast, track.name);

		// Get the track from the announce.
		let broadcast = self
			.announces
			.lock()
			.unwrap()
			.lookup
			.get(broadcast)
			.ok_or(Error::NotFound)?
			.clone();

		let mut source = broadcast.get_track(&track.name)?;

		// TODO only clone the fields we need
		let this = self.clone();

		tokio::spawn(async move {
			log::info!("serving track: broadcast={} track={}", broadcast.name, source.name,);

			let res = this.run_subscribe(id, &mut source, &mut track).await;
			if let Err(err) = &res {
				log::warn!(
					"failed to serve track: broadcast={} track={} err={:?}",
					broadcast.name,
					source.name,
					err
				);
			}

			let err = res.err().unwrap_or(Error::Closed);
			let msg = message::SubscribeReset {
				id,
				code: err.code(),
				reason: err.reason().to_string(),
			};

			this.control.send(msg).await.ok();
		});

		Ok(())
	}

	async fn run_subscribe(
		&self,
		id: VarInt,
		source: &mut track::Subscriber,
		_destination: &mut track::Publisher,
	) -> Result<(), Error> {
		// TODO add an Ok method to track::Publisher so we can send SUBSCRIBE_OK

		while let Some(mut segment) = source.next_segment().await? {
			// TODO only clone the fields we need
			let this = self.clone();

			tokio::spawn(async move {
				if let Err(err) = this.run_segment(id, &mut segment).await {
					log::warn!("failed to serve segment: {:?}", err)
				}
			});
		}

		Ok(())
	}

	async fn run_segment(&self, id: VarInt, segment: &mut segment::Subscriber) -> Result<(), Error> {
		let object = message::Object {
			track: id,
			sequence: segment.sequence,
			priority: segment.priority,
			expires: segment.expires,
		};

		log::debug!("serving object: {:?}", object);

		let mut stream = self.webtransport.open_uni().await.map_err(|_e| Error::Unknown)?;

		stream.set_priority(object.priority).ok();

		// TODO better handle the error.
		object.encode(&mut stream).await.map_err(|_e| Error::Unknown)?;

		while let Some(data) = segment.read_chunk().await? {
			stream.write_chunk(data).await.map_err(|_e| Error::Unknown)?;
		}

		Ok(())
	}

	async fn recv_subscribe_reset(&mut self, msg: &message::SubscribeReset) -> Result<(), Error> {
		let subscribe = self
			.subscribes
			.lock()
			.unwrap()
			.lookup
			.remove(&msg.id)
			.ok_or(Error::NotFound)?;

		subscribe.close(Error::Reset(msg.code))
	}
}
