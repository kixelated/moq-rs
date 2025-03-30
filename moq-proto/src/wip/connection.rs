use std::collections::HashMap;

use crate::{StreamId, StreamKind};

use super::{Publisher, Session, Subscriber};

pub struct Connection {
	session: Session,
	publisher: Publisher,
	subscriber: Subscriber,
}

impl Connection {
	// Create a new client connection.
	pub fn client() -> Self {
		Self {
			session: Session::new(true),
			publisher: Publisher::default(),
			subscriber: Subscriber::default(),
		}
	}

	// Create a new server connection.
	pub fn server() -> Self {
		Self {
			session: Session::new(false),
			publisher: Publisher::default(),
			subscriber: Subscriber::default(),
		}
	}

	pub fn session(&mut self) -> &mut Session {
		&mut self.session
	}

	pub fn publisher(&mut self) -> &mut Publisher {
		&mut self.publisher
	}

	pub fn subscriber(&mut self) -> &mut Subscriber {
		&mut self.subscriber
	}
}
