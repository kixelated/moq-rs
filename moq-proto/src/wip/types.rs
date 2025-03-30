use crate::message;

// message::Subscribe but without the ID.
pub struct SubscribeRequest {
	pub path: String,
	pub priority: i8,
	pub order: message::GroupOrder,
}

impl SubscribeRequest {
	pub fn into_message(self, id: u64) -> message::Subscribe {
		message::Subscribe {
			id,
			path: self.path,
			priority: self.priority,
			order: self.order,

			// TODO remove
			start: None,
			end: None,
		}
	}
}

impl From<message::Subscribe> for SubscribeRequest {
	fn from(msg: message::Subscribe) -> Self {
		Self {
			path: msg.path,
			priority: msg.priority,
			order: msg.order,
		}
	}
}
