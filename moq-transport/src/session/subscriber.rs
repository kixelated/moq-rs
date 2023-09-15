use webtransport_quinn::{RecvStream, SendStream, Session};

use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use crate::{
	message,
	message::Message,
	model::{broadcast, segment, track},
	Error, VarInt,
};

use super::Control;

/// Receives broadcasts over the network, automatically handling subscriptions and caching.
// TODO Clone specific fields when a task actually needs it.
#[derive(Clone, Debug)]
pub struct Subscriber {
	// The webtransport session.
	webtransport: Session,

	// The list of active subscriptions, each guarded by an mutex.
	subscribes: Arc<Mutex<HashMap<VarInt, track::Publisher>>>,

	// The sequence number for the next subscription.
	next: Arc<atomic::AtomicU32>,

	// A channel for sending messages.
	control: Control,

	// All unknown subscribes comes here.
	source: broadcast::Publisher,
}

impl Subscriber {
	pub(crate) fn new(webtransport: Session, control: (SendStream, RecvStream), source: broadcast::Publisher) -> Self {
		let control = Control::new(control.0, control.1);

		Self {
			webtransport,
			subscribes: Default::default(),
			next: Default::default(),
			control,
			source,
		}
	}

	pub async fn run(self) -> Result<(), Error> {
		let inbound = self.clone().run_inbound();
		let streams = self.clone().run_streams();
		let source = self.clone().run_source();

		// Return the first error.
		tokio::select! {
			res = inbound => res,
			res = streams => res,
			res = source => res,
		}
	}

	async fn run_inbound(mut self) -> Result<(), Error> {
		loop {
			let msg = self.control.recv().await.map_err(|_e| Error::Read)?;

			log::info!("message received: {:?}", msg);
			if let Err(err) = self.recv_message(&msg).await {
				log::warn!("message error: {:?} {:?}", err, msg);
			}
		}
	}

	async fn recv_message(&mut self, msg: &Message) -> Result<(), Error> {
		match msg {
			Message::Announce(_) => Ok(()),      // don't care
			Message::AnnounceReset(_) => Ok(()), // also don't care
			Message::SubscribeOk(_) => Ok(()),   // guess what, don't care
			Message::SubscribeReset(msg) => self.recv_subscribe_reset(msg).await,
			Message::GoAway(_msg) => unimplemented!("GOAWAY"),
			_ => Err(Error::Role(msg.id())),
		}
	}

	async fn recv_subscribe_reset(&mut self, msg: &message::SubscribeReset) -> Result<(), Error> {
		let err = Error::Reset(msg.code);

		let mut subscribes = self.subscribes.lock().unwrap();
		let subscribe = subscribes.remove(&msg.id).ok_or(Error::NotFound)?;
		subscribe.close(err)?;

		Ok(())
	}

	async fn run_streams(self) -> Result<(), Error> {
		loop {
			// Accept all incoming unidirectional streams.
			let stream = self.webtransport.accept_uni().await.map_err(|_| Error::Read)?;
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
			let track = subscribes.get_mut(&object.track).ok_or(Error::NotFound)?;

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

	async fn run_source(mut self) -> Result<(), Error> {
		while let Some(track) = self.source.next_track().await? {
			let name = track.name.clone();

			let id = VarInt::from_u32(self.next.fetch_add(1, atomic::Ordering::SeqCst));
			self.subscribes.lock().unwrap().insert(id, track);

			let msg = message::Subscribe {
				id,
				namespace: "".to_string(),
				name,
			};

			self.control.send(msg).await?;
		}

		Ok(())
	}
}
