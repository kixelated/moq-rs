use std::{
	collections::HashMap,
	sync::{atomic, Arc, Mutex},
};

use futures::{stream::FuturesUnordered, StreamExt};

use crate::{
	message,
	serve::{self, ServeError},
	session::Control,
	setup,
};

use crate::util::Queue;

use super::{Announced, Reader, Session, SessionError, Subscribe};

#[derive(Clone)]
pub struct Subscriber {
	webtransport: web_transport::Session,
	announced: Queue<Announced>,
	subscribed: Queue<Subscribe>,

	lookup: Arc<Mutex<HashMap<u64, serve::TrackWriter>>>,
	next_id: Arc<atomic::AtomicU64>,
}

impl Subscriber {
	pub(super) fn new(webtransport: web_transport::Session) -> Self {
		Self {
			webtransport,
			announced: Default::default(),
			subscribed: Default::default(),
			lookup: Default::default(),
			next_id: Default::default(),
		}
	}

	pub async fn accept(session: web_transport::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::accept_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::connect_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn announced(&mut self) -> Option<Announced> {
		self.announced.pop().await
	}

	pub fn subscribe(&mut self, track: serve::TrackWriter) -> Result<Subscribe, SessionError> {
		let id = self.next_id.fetch_add(1, atomic::Ordering::Relaxed);

		let msg = message::Subscribe {
			id,
			broadcast: track.broadcast.to_string(),
			track: track.name.clone(),

			// TODO
			priority: track.priority.unwrap_or(0),
			order: track.order.map(Into::into),
			expires: None,
			min: None,
			max: None,
		};

		// Insert into our lookup before we send the message
		self.lookup.lock().unwrap().insert(id, track);

		// Once we have successfully subscribed, we return a Subscribe object to the application.
		let subscribe = Subscribe::new(msg);
		if let Err(_) = self.subscribed.push(subscribe.split()) {
			return Err(SessionError::Internal);
		}

		Ok(subscribe)
	}

	pub(super) async fn run(&mut self) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(subscribed) = self.subscribed.pop() => {
					let mut webtransport = self.webtransport.clone();
					tasks.push(async move {
						let control = Control::open(&mut webtransport, message::Control::Subscribe).await?;
						subscribed.run(control).await
					});
				},
				res = tasks.next(), if !tasks.is_empty() => {
					match res.unwrap() {
						Ok(id) => self.lookup.lock().unwrap().remove(&id),
						Err(err) => return Err(err),
					};
				},
				else => return Ok(()),
			};
		}
	}

	pub(super) async fn run_announce(&mut self, mut control: Control) -> Result<(), SessionError> {
		let msg: message::Announce = control.reader.decode().await?;

		let announced = Announced::new(self.clone(), msg);
		let _ = self.announced.push(announced.split());

		announced.run(control).await
	}

	pub(super) async fn run_group(&mut self, mut reader: Reader) -> Result<(), SessionError> {
		let header: message::Group = reader.decode().await?;

		let group = serve::Group {
			sequence: header.sequence,
			expires: header.expires,
		};

		let mut group = self
			.lookup
			.lock()
			.unwrap()
			.get_mut(&header.subscribe)
			.ok_or(ServeError::NotFound)?
			.insert(group)?;

		while let Some(chunk) = reader.read_chunk(usize::MAX).await? {
			group.write(chunk)?;
		}

		Ok(())
	}
}
