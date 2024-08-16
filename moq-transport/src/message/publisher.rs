use crate::message::{self, Message};
use std::fmt;

macro_rules! publisher_msgs {
    {$($name:ident,)*} => {
		#[derive(Clone)]
		pub enum Publisher {
			$($name(message::$name)),*
		}

		$(impl From<message::$name> for Publisher {
			fn from(msg: message::$name) -> Self {
				Publisher::$name(msg)
			}
		})*

		impl From<Publisher> for Message {
			fn from(p: Publisher) -> Self {
				match p {
					$(Publisher::$name(m) => Message::$name(m),)*
				}
			}
		}

		impl TryFrom<Message> for Publisher {
			type Error = Message;

			fn try_from(m: Message) -> Result<Self, Self::Error> {
				match m {
					$(Message::$name(m) => Ok(Publisher::$name(m)),)*
					_ => Err(m),
				}
			}
		}

		impl fmt::Debug for Publisher {
			// Delegate to the message formatter
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				match self {
					$(Self::$name(ref m) => m.fmt(f),)*
				}
			}
		}
    }
}

publisher_msgs! {
	Announce,
	Unannounce,
	SubscribeOk,
	SubscribeError,
	SubscribeDone,
	TrackStatus,
}
