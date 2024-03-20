use crate::message::{self, Message};

macro_rules! subscriber_msgs {
    {$($name:ident,)*} => {
		#[derive(Clone, Debug)]
		pub enum Subscriber {
			$($name(message::$name)),*
		}

		$(impl Into<Subscriber> for message::$name {
			fn into(self) -> Subscriber {
				Subscriber::$name(self)
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
    }
}

subscriber_msgs! {
	AnnounceOk,
	AnnounceError,
	AnnounceCancel,
	Subscribe,
	Unsubscribe,
}
