use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use tokio::task::AbortHandle;
use webtransport_quinn::Session;

use crate::{
	cache::{broadcast, segment, track, CacheError},
	message,
	message::Message,
	MoqError, VarInt,
};

use super::{Control, SessionError};

/// Serves broadcasts over the network, automatically handling subscriptions and caching.
// TODO Clone specific fields when a task actually needs it.
#[derive(Clone, Debug)]
pub struct Publisher {
	// A map of active subscriptions, containing an abort handle to cancel them.
	subscribes: Arc<Mutex<HashMap<VarInt, AbortHandle>>>,
	webtransport: Session,
	control: Control,
	source: broadcast::Subscriber,
}

impl Publisher {
	pub(crate) fn new(webtransport: Session, control: Control, source: broadcast::Subscriber) -> Self {
		Self {
			webtransport,
			control,
			subscribes: Default::default(),
			source,
		}
	}

	// TODO Serve a broadcast without sending an ANNOUNCE.
	// fn serve(&mut self, broadcast: broadcast::Subscriber) -> Result<(), SessionError> {

	// TODO Wait until the next subscribe that doesn't route to an ANNOUNCE.
	// pub async fn subscribed(&mut self) -> Result<track::Producer, SessionError> {

	pub async fn run(mut self) -> Result<(), SessionError> {
		let res = self.run_inner().await;

		// Terminate all active subscribes on error.
		self.subscribes
			.lock()
			.unwrap()
			.drain()
			.for_each(|(_, abort)| abort.abort());

		res
	}

	pub async fn run_inner(&mut self) -> Result<(), SessionError> {
		loop {
			tokio::select! {
				stream = self.webtransport.accept_uni() => {
					stream?;
					return Err(SessionError::RoleViolation(VarInt::ZERO));
				}
				// NOTE: this is not cancel safe, but it's fine since the other branchs are fatal.
				msg = self.control.recv() => {
					let msg = msg?;

					log::info!("message received: {:?}", msg);
					if let Err(err) = self.recv_message(&msg).await {
						log::warn!("message error: {:?} {:?}", err, msg);
					}
				},
				// No more broadcasts are available.
				err = self.source.closed() => {
					self.webtransport.close(err.code(), err.reason().as_bytes());
					return Ok(());
				},
			}
		}
	}

	async fn recv_message(&mut self, msg: &Message) -> Result<(), SessionError> {
		match msg {
			Message::AnnounceOk(msg) => self.recv_announce_ok(msg).await,
			Message::AnnounceError(msg) => self.recv_announce_error(msg).await,
			Message::Subscribe(msg) => self.recv_subscribe(msg).await,
			Message::Unsubscribe(msg) => self.recv_unsubscribe(msg).await,
			_ => Err(SessionError::RoleViolation(msg.id())),
		}
	}

	async fn recv_announce_ok(&mut self, _msg: &message::AnnounceOk) -> Result<(), SessionError> {
		// We didn't send an announce.
		Err(CacheError::NotFound.into())
	}

	async fn recv_announce_error(&mut self, _msg: &message::AnnounceError) -> Result<(), SessionError> {
		// We didn't send an announce.
		Err(CacheError::NotFound.into())
	}

	async fn recv_subscribe(&mut self, msg: &message::Subscribe) -> Result<(), SessionError> {
		// Assume that the subscribe ID is unique for now.
		let abort = match self.start_subscribe(msg.clone()) {
			Ok(abort) => abort,
			Err(err) => return self.reset_subscribe(msg.id, err).await,
		};

		// Insert the abort handle into the lookup table.
		match self.subscribes.lock().unwrap().entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(CacheError::Duplicate.into()), // TODO fatal, because we already started the task
			hash_map::Entry::Vacant(entry) => entry.insert(abort),
		};

		self.control
			.send(message::SubscribeOk {
				id: msg.id,
				expires: VarInt::ZERO,
			})
			.await
	}

	async fn reset_subscribe<E: MoqError>(&mut self, id: VarInt, err: E) -> Result<(), SessionError> {
		let msg = message::SubscribeReset {
			id,
			code: err.code(),
			reason: err.reason(),

			// TODO properly populate these
			// But first: https://github.com/moq-wg/moq-transport/issues/313
			final_group: VarInt::ZERO,
			final_object: VarInt::ZERO,
		};

		self.control.send(msg).await
	}

	fn start_subscribe(&mut self, msg: message::Subscribe) -> Result<AbortHandle, SessionError> {
		// We currently don't use the namespace field in SUBSCRIBE
		// Make sure the namespace is empty if it's provided.
		if msg.namespace.as_ref().map_or(false, |namespace| !namespace.is_empty()) {
			return Err(CacheError::NotFound.into());
		}

		let mut track = self.source.get_track(&msg.name)?;

		// TODO only clone the fields we need
		let mut this = self.clone();

		let handle = tokio::spawn(async move {
			log::info!("serving track: name={}", track.name);

			let res = this.run_subscribe(msg.id, &mut track).await;
			if let Err(err) = &res {
				log::warn!("failed to serve track: name={} err={:#?}", track.name, err);
			}

			// Make sure we send a reset at the end.
			let err = res.err().unwrap_or(CacheError::Closed.into());
			this.reset_subscribe(msg.id, err).await.ok();

			// We're all done, so clean up the abort handle.
			this.subscribes.lock().unwrap().remove(&msg.id);
		});

		Ok(handle.abort_handle())
	}

	async fn run_subscribe(&self, id: VarInt, track: &mut track::Subscriber) -> Result<(), SessionError> {
		// TODO add an Ok method to track::Publisher so we can send SUBSCRIBE_OK

		while let Some(mut segment) = track.segment().await? {
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

	async fn run_segment(&self, id: VarInt, segment: &mut segment::Subscriber) -> Result<(), SessionError> {
		log::trace!("serving group: {:?}", segment);

		let mut stream = self.webtransport.open_uni().await?;

		// Convert the u32 to a i32, since the Quinn set_priority is signed.
		let priority = (segment.priority as i64 - i32::MAX as i64) as i32;
		stream.set_priority(priority).ok();

		while let Some(mut fragment) = segment.fragment().await? {
			log::trace!("serving fragment: {:?}", fragment);

			let object = message::Object {
				track: id,

				// Properties of the segment
				group: segment.sequence,
				priority: segment.priority,
				expires: segment.expires,

				// Properties of the fragment
				sequence: fragment.sequence,
				size: fragment.size.map(VarInt::try_from).transpose()?,
			};

			object
				.encode(&mut stream, &self.control.ext)
				.await
				.map_err(|e| SessionError::Unknown(e.to_string()))?;

			while let Some(chunk) = fragment.chunk().await? {
				//log::trace!("writing chunk: {:?}", chunk);
				stream.write_all(&chunk).await?;
			}
		}

		Ok(())
	}

	async fn recv_unsubscribe(&mut self, msg: &message::Unsubscribe) -> Result<(), SessionError> {
		let abort = self
			.subscribes
			.lock()
			.unwrap()
			.remove(&msg.id)
			.ok_or(CacheError::NotFound)?;
		abort.abort();

		self.reset_subscribe(msg.id, CacheError::Stop).await
	}
}
