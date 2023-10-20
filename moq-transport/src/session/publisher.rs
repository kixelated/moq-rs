use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use tokio::task::AbortHandle;
use webtransport_quinn::{RecvStream, SendStream, Session};

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
	pub(crate) fn new(webtransport: Session, control: (SendStream, RecvStream), source: broadcast::Subscriber) -> Self {
		let control = Control::new(control.0, control.1);

		Self {
			webtransport,
			subscribes: Default::default(),
			control,
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
			Message::AnnounceStop(msg) => self.recv_announce_stop(msg).await,
			Message::Subscribe(msg) => self.recv_subscribe(msg).await,
			Message::SubscribeStop(msg) => self.recv_subscribe_stop(msg).await,
			_ => Err(SessionError::RoleViolation(msg.id())),
		}
	}

	async fn recv_announce_ok(&mut self, _msg: &message::AnnounceOk) -> Result<(), SessionError> {
		// We didn't send an announce.
		Err(CacheError::NotFound.into())
	}

	async fn recv_announce_stop(&mut self, _msg: &message::AnnounceStop) -> Result<(), SessionError> {
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

		self.control.send(message::SubscribeOk { id: msg.id }).await
	}

	async fn reset_subscribe<E: MoqError>(&mut self, id: VarInt, err: E) -> Result<(), SessionError> {
		let msg = message::SubscribeReset {
			id,
			code: err.code(),
			reason: err.reason().to_string(),
		};

		self.control.send(msg).await
	}

	fn start_subscribe(&mut self, msg: message::Subscribe) -> Result<AbortHandle, SessionError> {
		// We currently don't use the namespace field in SUBSCRIBE
		if !msg.namespace.is_empty() {
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

		while let Some(mut segment) = track.next_segment().await? {
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
		let object = message::Object {
			track: id,
			sequence: segment.sequence,
			priority: segment.priority,
			expires: segment.expires,
		};

		log::trace!("serving object: {:?}", object);

		let mut stream = self.webtransport.open_uni().await?;
		stream.set_priority(object.priority).ok();

		object
			.encode(&mut stream)
			.await
			.map_err(|e| SessionError::Unknown(e.to_string()))?;

		while let Some(data) = segment.read_chunk().await? {
			stream.write_chunk(data).await?;
		}

		Ok(())
	}

	async fn recv_subscribe_stop(&mut self, msg: &message::SubscribeStop) -> Result<(), SessionError> {
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
