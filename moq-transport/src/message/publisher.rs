use crate::message::{self, Message};

macro_rules! publisher_msgs {
    {$($name:ident,)*} => {
		#[derive(Clone, Debug)]
		pub enum Publisher {
			$($name(message::$name)),*
		}

		$(impl Into<Publisher> for message::$name {
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

publisher_msgs! {
	Announce,
	Unannounce,
	SubscribeOk,
	SubscribeError,
	SubscribeDone,
}
