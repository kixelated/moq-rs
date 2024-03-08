use webtransport_quinn::{RecvStream, Session};

use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use crate::{
	cache::{broadcast, segment, track, CacheError},
	coding::DecodeError,
	control,
	control::Message,
	data,
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
			Message::SubscribeDone(msg) => self.recv_subscribe_error(msg.id, CacheError::Reset(msg.code.try_into()?)),
			Message::SubscribeError(msg) => self.recv_subscribe_error(msg.id, CacheError::Reset(msg.code.try_into()?)),
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
					log::warn!("failed to receive stream: err={}", err);
				}
			});
		}
	}

	async fn run_stream(self, mut stream: RecvStream) -> Result<(), SessionError> {
		// Decode the object on the data stream.
		let header = data::Header::decode(&mut stream)
			.await
			.map_err(|e| SessionError::Unknown(e.to_string()))?;

		log::trace!("receiving stream: {:?}", header);

		match header {
			data::Header::Track(header) => self.run_track(header, stream).await,
			data::Header::Group(header) => self.run_group(header, stream).await,
			data::Header::Object(header) => self.run_object(header, stream).await,
			data::Header::Datagram(_) => Err(SessionError::InvalidMessage),
		}
	}

	async fn run_track(self, header: data::Track, mut stream: RecvStream) -> Result<(), SessionError> {
		loop {
			let chunk = match data::TrackChunk::decode(&mut stream).await {
				Ok(next) => next,

				// TODO Figure out a way to check for stream FIN instead
				Err(DecodeError::UnexpectedEnd) => break,

				// Unknown error
				Err(err) => return Err(err.into()),
			};

			log::trace!("receiving chunk: {:?}", chunk);

			// TODO error if we get a duplicate group
			let mut segment = {
				let mut subscribes = self.subscribes.lock().unwrap();
				let track = subscribes.get_mut(&header.subscribe_id).ok_or(CacheError::NotFound)?;

				track.create_segment(segment::Info {
					sequence: chunk.group_id,
					priority: header.send_order.try_into()?, // TODO support u64 priorities
				})?
			};

			let mut remain = chunk.size.into();

			// Create a new obvject.
			let mut fragment = segment.fragment(remain)?;

			while remain > 0 {
				let data = stream
					.read_chunk(remain, true)
					.await?
					.ok_or(DecodeError::UnexpectedEnd)?
					.bytes;

				log::trace!("read data: len={}", data.len());
				remain -= data.len();
				fragment.chunk(data)?;
			}
		}

		Ok(())
	}

	async fn run_group(self, header: data::Group, mut stream: RecvStream) -> Result<(), SessionError> {
		let mut segment = {
			let mut subscribes = self.subscribes.lock().unwrap();
			let track = subscribes.get_mut(&header.subscribe_id).ok_or(CacheError::NotFound)?;

			track.create_segment(segment::Info {
				sequence: header.group_id,
				priority: header.send_order.try_into()?, // TODO support u64 priorities
			})?
		};

		// Sanity check to make sure we receive the group in order
		let mut expected = 0;

		loop {
			let chunk = match data::GroupChunk::decode(&mut stream).await {
				Ok(chunk) => chunk,

				// TODO Figure out a way to check for stream FIN instead
				Err(DecodeError::UnexpectedEnd) => break,

				// Unknown error
				Err(err) => return Err(err.into()),
			};

			log::trace!("receiving chunk: {:?}", chunk);

			// NOTE: We allow gaps, but not going backwards.
			if chunk.object_id.into_inner() < expected {
				return Err(SessionError::OutOfOrder(expected.try_into()?, chunk.object_id));
			}

			expected = chunk.object_id.into_inner() + 1;

			let mut remain = chunk.size.into();

			// Create a new obvject.
			let mut fragment = segment.fragment(remain)?;

			while remain > 0 {
				let data = stream
					.read_chunk(remain, true)
					.await?
					.ok_or(DecodeError::UnexpectedEnd)?;
				remain -= data.bytes.len();
				fragment.chunk(data.bytes)?;
			}
		}

		Ok(())
	}

	async fn run_object(self, _header: data::Object, _stream: RecvStream) -> Result<(), SessionError> {
		unimplemented!("TODO");
	}

	async fn run_source(mut self) -> Result<(), SessionError> {
		loop {
			// NOTE: This returns Closed when the source is closed.
			let track = self.source.next_track().await?;
			let track_name = track.name.clone();

			let id = VarInt::from_u32(self.next.fetch_add(1, atomic::Ordering::SeqCst));
			self.subscribes.lock().unwrap().insert(id, track);

			let msg = control::Subscribe {
				id,

				track_alias: id, // This alias is useless but part of the spec.
				track_namespace: "".to_string(),
				track_name,

				// TODO correctly support these
				start_group: control::SubscribeLocation::Latest(VarInt::ZERO),
				start_object: control::SubscribeLocation::Absolute(VarInt::ZERO),
				end_group: control::SubscribeLocation::None,
				end_object: control::SubscribeLocation::None,

				params: Default::default(),
			};

			self.control.send(msg).await?;
		}
	}
}
