use crate::control::{self, Message};

macro_rules! publisher_msgs {
    {$($name:ident,)*} => {
		#[derive(Clone)]
		pub enum Publisher {
			$($name(control::$name)),*
		}

		$(impl Into<Publisher> for control::$name {
			fn into(self) -> Publisher {
				Publisher::$name(self)
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
    }
}
macro_rules! subscriber_msgs {
    {$($name:ident,)*} => {
		#[derive(Clone)]
		pub enum Subscriber {
			$($name(control::$name)),*
		}

		$(impl Into<Subscriber> for control::$name {
			fn into(self) -> Subscriber{
				Subscriber::$name(self)
			}
		})*

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

publisher_msgs! {
	Announce,
	Unannounce,
	SubscribeOk,
	SubscribeError,
	SubscribeDone,
}

subscriber_msgs! {
	AnnounceOk,
	AnnounceError,
	AnnounceCancel,
	Subscribe,
	Unsubscribe,
}
