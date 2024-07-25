use crate::message::{self, Message};
use std::fmt;

macro_rules! subscriber_msgs {
    {$($name:ident,)*} => {
		#[derive(Clone)]
		pub enum Subscriber {
			$($name(message::$name)),*
		}

		$(impl From<message::$name> for Subscriber {
			fn from(msg: message::$name) -> Self {
				Subscriber::$name(msg)
			}
		})*

		impl From<Subscriber> for Message {
			fn from(p: Subscriber) -> Self {
				match p {
					$(Subscriber::$name(m) => Message::$name(m),)*
				}
			}
		}

		impl TryFrom<Message> for Subscriber {
			type Error = Message;

			fn try_from(m: Message) -> Result<Self, Self::Error> {
				match m {
					$(Message::$name(m) => Ok(Subscriber::$name(m)),)*
					_ => Err(m),
				}
			}
		}

		impl fmt::Debug for Subscriber {
			// Delegate to the message formatter
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				match self {
					$(Self::$name(ref m) => m.fmt(f),)*
				}
			}
		}
    }
}

subscriber_msgs! {
	AnnounceOk,
	AnnounceError,
	AnnounceCancel,
	Subscribe,
	Unsubscribe,
	SubscribeUpdate,
	TrackStatusRequest,
}
