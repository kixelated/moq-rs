use crate::message;

use super::GroupId;

// message::Subscribe but without the ID.
pub struct SubscribeRequest {
	pub path: String,
	pub priority: i8,
	pub group_order: message::GroupOrder,
	pub group_min: Option<GroupId>,
	pub group_max: Option<GroupId>,
}

impl SubscribeRequest {
	pub fn into_message(self, id: u64) -> message::Subscribe {
		message::Subscribe {
			id,
			path: self.path,
			priority: self.priority,
			group_order: self.group_order,
			group_min: self.group_min.map(Into::into),
			group_max: self.group_max.map(Into::into),
		}
	}
}

impl From<message::Subscribe> for SubscribeRequest {
	fn from(msg: message::Subscribe) -> Self {
		Self {
			path: msg.path,
			priority: msg.priority,
			group_order: msg.group_order,
			group_min: msg.group_min.map(Into::into),
			group_max: msg.group_max.map(Into::into),
		}
	}
}
