use derive_more::From;

use super::{
	Publisher, PublisherEvent, PublisherState, Session, SessionEvent, SessionState, StreamDirection, StreamEvent,
	Streams, StreamsState, Subscriber, SubscriberEvent, SubscriberState,
};

pub struct Connection {
	session: SessionState,
	publisher: PublisherState,
	subscriber: SubscriberState,
	streams: StreamsState,
}

impl Connection {
	// Create a new client connection.
	pub fn client() -> Self {
		Self {
			session: SessionState::new(true),
			publisher: PublisherState::default(),
			subscriber: SubscriberState::default(),
			streams: StreamsState::default(),
		}
	}

	// Create a new server connection.
	pub fn server() -> Self {
		Self {
			session: SessionState::new(false),
			publisher: PublisherState::default(),
			subscriber: SubscriberState::default(),
			streams: StreamsState::default(),
		}
	}

	pub fn streams(&mut self) -> Streams {
		Streams {
			state: &mut self.streams,
			session: &mut self.session,
			publisher: &mut self.publisher,
			subscriber: &mut self.subscriber,
		}
	}

	pub fn session(&mut self) -> Session {
		Session {
			state: &mut self.session,
		}
	}

	pub fn publisher(&mut self) -> Publisher {
		Publisher {
			state: &mut self.publisher,
			streams: &mut self.streams,
		}
	}

	pub fn subscriber(&mut self) -> Subscriber {
		Subscriber {
			state: &mut self.subscriber,
			streams: &mut self.streams,
		}
	}
}
