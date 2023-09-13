use webtransport_quinn::{RecvStream, SendStream, Session};

use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use crate::{
	message,
	message::Message,
	model::{broadcast, broker, segment, track},
	Error, VarInt,
};

use super::Control;

#[derive(Debug)]
struct Announces {
	// Write new announces to the publisher.
	publisher: broker::Publisher,

	// Keep a subscriber that we can clone and return for announced().
	subscriber: broker::Subscriber,
}

impl Default for Announces {
	fn default() -> Self {
		let (publisher, subscriber) = broker::new();
		Self { publisher, subscriber }
	}
}

#[derive(Default, Debug)]
struct Subscribes {
	// A lookup from ID to Subscribe.
	// If the value is smaller than next but not in the lookup, then it have been closed.
	tracks: HashMap<VarInt, track::Publisher>,

	// The sequence number for the next subscription.
	next: u32,
}

/// Receives broadcasts over the network, automatically handling subscriptions and caching.
#[derive(Clone, Debug)]
pub struct Subscriber {
	// The webtransport session.
	webtransport: Session,

	// The list of active announces and subscriptions, each guarded by an mutex.
	announces: Arc<Mutex<Announces>>,
	subscribes: Arc<Mutex<Subscribes>>,

	// A channel for sending messages.
	control: Control,
}

impl Subscriber {
	pub(crate) fn new(webtransport: Session, control: (SendStream, RecvStream)) -> Self {
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

	/// Return a subscriber that emits each new broadcast announced.
	/// This can be called multiple times and all announces will be fanned out.
	pub fn announced(&self) -> broker::Subscriber {
		// TODO move this out of the mutex.
		self.announces.lock().unwrap().subscriber.clone()
	}

	/// Subscribe to a track without an explicit ANNOUNCE.
	pub async fn subscribe(&self, broadcast: &str, track: track::Publisher) -> Result<(), Error> {
		let name = track.name.clone();

		// Have to use a temporary scope because the compiler isn't smart enough to detect Send
		let id = {
			let mut subscribes = self.subscribes.lock().unwrap();

			let id = VarInt::from_u32(subscribes.next);
			subscribes.next += 1;
			subscribes.tracks.insert(id, track);

			id
		};

		let msg = message::Subscribe {
			id,
			namespace: broadcast.to_string(),
			name,
		};

		self.control.send(msg).await?;

		Ok(())
	}

	/// Close the WebTransport session.
	pub fn close(self, code: u32, reason: &str) {
		self.webtransport.close(code, reason.as_bytes())
	}

	// Private API
	async fn run(self, control: RecvStream) -> Result<(), Error> {
		let inbound = self.clone().run_inbound(control);
		let streams = self.clone().run_streams();

		// Return the first error.
		tokio::select! {
			res = inbound => res,
			res = streams => res,
		}
	}

	async fn run_inbound(mut self, mut control: RecvStream) -> Result<(), Error> {
		loop {
			let msg = Message::decode(&mut control).await.map_err(|_e| Error::Unknown)?;

			log::info!("message received: {:?}", msg);
			if let Err(err) = self.recv_message(&msg).await {
				log::warn!("message error: {:?} {:?}", err, msg);
			}
		}
	}

	async fn recv_message(&mut self, msg: &Message) -> Result<(), Error> {
		match msg {
			Message::Announce(msg) => self.recv_announce(msg).await,
			Message::AnnounceReset(msg) => self.recv_announce_reset(msg).await,
			Message::SubscribeOk(msg) => self.recv_subscribe_ok(msg).await,
			Message::SubscribeStop(msg) => self.recv_subscribe_stop(msg).await,
			Message::GoAway(_msg) => unimplemented!("GOAWAY"),
			_ => Err(Error::Role(msg.id())),
		}
	}

	async fn recv_announce(&mut self, msg: &message::Announce) -> Result<(), Error> {
		let name = msg.namespace.clone();

		// TODO keep a reference to Producer (currently _) so we can close it on ANNOUNCE_RESET to close Unknown.
		let (_, mut unknown) = self.announces.lock().unwrap().publisher.create_broadcast(&name)?;

		let mut this = self.clone();

		// Start a task to automatically subscriptions while the ANNOUNCE is active.
		tokio::spawn(async move {
			log::info!("serving broadcast: name={}", name);
			if let Err(err) = this.serve_broadcast(&mut unknown).await {
				log::warn!("serving broadcast error: name={} err={:?}", name, err);
			}

			// We don't send an ANNOUNCE STOP because it's useless TBH
		});

		let msg = message::AnnounceOk {
			namespace: msg.namespace.clone(),
		};

		self.control.send(msg).await?;

		Ok(())
	}

	async fn serve_broadcast(&mut self, unknown: &mut broadcast::Unknown) -> Result<(), Error> {
		// Wait until the subscriber requests a new track.
		while let Some(track) = unknown.next_track().await? {
			// Subscribe to the track to fufill the request.
			self.subscribe(&unknown.name, track).await?;
		}

		// No more subscribers, so we can stop this task.
		Ok(())
	}

	async fn recv_announce_reset(&mut self, msg: &message::AnnounceReset) -> Result<(), Error> {
		// Remove the announce.
		// TODO signal serve_broadcast to terminate immediately, instead of waiting for all subscribers to exit.
		self.announces
			.lock()
			.unwrap()
			.publisher
			.remove_broadcast(&msg.namespace)
	}

	async fn recv_subscribe_ok(&mut self, _msg: &message::SubscribeOk) -> Result<(), Error> {
		// who cares
		Ok(())
	}

	async fn recv_subscribe_stop(&mut self, msg: &message::SubscribeStop) -> Result<(), Error> {
		let err = Error::Stop(msg.code);

		// We need a new scope because the async compiler is dumb
		{
			let mut subscribes = self.subscribes.lock().unwrap();
			let subscribe = subscribes.tracks.remove(&msg.id).ok_or(Error::NotFound)?;
			subscribe.close(err)?;
		}

		// Send the RESET now.
		let msg = message::SubscribeReset {
			id: msg.id,
			code: msg.code,
			reason: err.reason().to_string(),
		};

		self.control.send(msg).await?;

		Ok(())
	}

	async fn run_streams(self) -> Result<(), Error> {
		loop {
			// Accept all incoming unidirectional streams.
			let stream = self.webtransport.accept_uni().await.map_err(|_| Error::Unknown)?;
			let this = self.clone();

			tokio::spawn(async move {
				if let Err(err) = this.run_stream(stream).await {
					log::warn!("failed to receive stream: err={:?}", err);
				}
			});
		}
	}

	async fn run_stream(self, mut stream: RecvStream) -> Result<(), Error> {
		// Decode the object on the data stream.
		let object = message::Object::decode(&mut stream).await.map_err(|_| Error::Read)?;

		log::debug!("received object: {:?}", object);

		// A new scope is needed because the async compiler is dumb
		let mut publisher = {
			let mut subscribes = self.subscribes.lock().unwrap();
			let track = subscribes.tracks.get_mut(&object.track).ok_or(Error::NotFound)?;

			track.create_segment(segment::Info {
				sequence: object.sequence,
				priority: object.priority,
				expires: object.expires,
			})?
		};

		while let Some(data) = stream.read_chunk(usize::MAX, true).await.map_err(|_| Error::Read)? {
			publisher.write_chunk(data.bytes)?;
		}

		Ok(())
	}
}
