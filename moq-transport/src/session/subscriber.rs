use webtransport_quinn::{RecvStream, Session};

use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use crate::{
	cache::{broadcast, segment, track, CacheError},
	coding::DecodeError,
	message,
	message::Message,
	session::{Control, SessionError},
	VarInt,
};

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
	pub(crate) fn new(webtransport: Session, control: Control, source: broadcast::Publisher) -> Self {
		Self {
			webtransport,
			subscribes: Default::default(),
			next: Default::default(),
			control,
			source,
		}
	}

	pub async fn run(self) -> Result<(), SessionError> {
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

	async fn run_inbound(mut self) -> Result<(), SessionError> {
		loop {
			let msg = self.control.recv().await?;

			log::info!("message received: {:?}", msg);
			if let Err(err) = self.recv_message(&msg) {
				log::warn!("message error: {:?} {:?}", err, msg);
			}
		}
	}

	fn recv_message(&mut self, msg: &Message) -> Result<(), SessionError> {
		match msg {
			Message::Announce(_) => Ok(()),       // don't care
			Message::Unannounce(_) => Ok(()),     // also don't care
			Message::SubscribeOk(_msg) => Ok(()), // don't care
			Message::SubscribeReset(msg) => self.recv_subscribe_error(msg.id, CacheError::Reset(msg.code)),
			Message::SubscribeFin(msg) => self.recv_subscribe_error(msg.id, CacheError::Closed),
			Message::SubscribeError(msg) => self.recv_subscribe_error(msg.id, CacheError::Reset(msg.code)),
			Message::GoAway(_msg) => unimplemented!("GOAWAY"),
			_ => Err(SessionError::RoleViolation(msg.id())),
		}
	}

	fn recv_subscribe_error(&mut self, id: VarInt, err: CacheError) -> Result<(), SessionError> {
		let mut subscribes = self.subscribes.lock().unwrap();
		let subscribe = subscribes.remove(&id).ok_or(CacheError::NotFound)?;
		subscribe.close(err)?;

		Ok(())
	}

	async fn run_streams(self) -> Result<(), SessionError> {
		loop {
			// Accept all incoming unidirectional streams.
			let stream = self.webtransport.accept_uni().await?;
			let this = self.clone();

			tokio::spawn(async move {
				if let Err(err) = this.run_stream(stream).await {
					log::warn!("failed to receive stream: err={:#?}", err);
				}
			});
		}
	}

	async fn run_stream(self, mut stream: RecvStream) -> Result<(), SessionError> {
		// Decode the object on the data stream.
		let mut object = message::Object::decode(&mut stream, &self.control.ext)
			.await
			.map_err(|e| SessionError::Unknown(e.to_string()))?;

		log::trace!("first object: {:?}", object);

		// A new scope is needed because the async compiler is dumb
		let mut segment = {
			let mut subscribes = self.subscribes.lock().unwrap();
			let track = subscribes.get_mut(&object.track).ok_or(CacheError::NotFound)?;

			track.create_segment(segment::Info {
				sequence: object.group,
				priority: object.priority,
				expires: object.expires,
			})?
		};

		log::trace!("received segment: {:?}", segment);

		// Create the first fragment
		let mut fragment = segment.push_fragment(object.sequence, object.size.map(usize::from))?;
		let mut remain = object.size.map(usize::from);

		loop {
			if let Some(0) = remain {
				// Decode the next object from the stream.
				let next = match message::Object::decode(&mut stream, &self.control.ext).await {
					Ok(next) => next,

					// No more objects
					Err(DecodeError::Final) => break,

					// Unknown error
					Err(err) => return Err(err.into()),
				};

				log::trace!("next object: {:?}", object);

				// NOTE: This is a custom restriction; not part of the moq-transport draft.
				// We require every OBJECT to contain the same priority since prioritization is done per-stream.
				// We also require every OBJECT to contain the same group so we know when the group ends, and can detect gaps.
				if next.priority != object.priority && next.group != object.group {
					return Err(SessionError::StreamMapping);
				}

				object = next;

				// Create a new object.
				fragment = segment.push_fragment(object.sequence, object.size.map(usize::from))?;
				remain = object.size.map(usize::from);

				log::trace!("next fragment: {:?}", fragment);
			}

			match stream.read_chunk(remain.unwrap_or(usize::MAX), true).await? {
				// Unbounded object has ended
				None if remain.is_none() => break,

				// Bounded object ended early, oops.
				None => return Err(DecodeError::UnexpectedEnd.into()),

				// NOTE: This does not make a copy!
				// Bytes are immutable and ref counted.
				Some(data) => {
					remain = remain.map(|r| r - data.bytes.len());

					log::trace!("next chunk: {:?}", data);
					fragment.chunk(data.bytes)?;
				}
			}
		}

		Ok(())
	}

	async fn run_source(mut self) -> Result<(), SessionError> {
		loop {
			// NOTE: This returns Closed when the source is closed.
			let track = self.source.next_track().await?;
			let name = track.name.clone();

			let id = VarInt::from_u32(self.next.fetch_add(1, atomic::Ordering::SeqCst));
			self.subscribes.lock().unwrap().insert(id, track);

			let msg = message::Subscribe {
				id,
				namespace: self.control.ext.subscribe_split.then(|| "".to_string()),
				name,

				// TODO correctly support these
				start_group: message::SubscribeLocation::Latest(VarInt::ZERO),
				start_object: message::SubscribeLocation::Absolute(VarInt::ZERO),
				end_group: message::SubscribeLocation::None,
				end_object: message::SubscribeLocation::None,

				params: Default::default(),
			};

			self.control.send(msg).await?;
		}
	}
}
