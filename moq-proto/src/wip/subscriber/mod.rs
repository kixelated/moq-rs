mod announce;
mod group;
mod subscribe;

pub use announce::*;
pub use group::*;
pub use subscribe::*;

#[derive(Default)]
pub struct Subscriber {
	announces: SubscriberAnnounces,
	subscribes: SubscriberSubscribes,
	groups: SubscriberGroups,
}

impl Subscriber {
	pub fn announces(&mut self) -> &mut SubscriberAnnounces {
		&mut self.announces
	}

	pub fn subscribes(&mut self) -> &mut SubscriberSubscribes {
		&mut self.subscribes
	}

	pub fn groups(&mut self) -> &mut SubscriberGroups {
		&mut self.groups
	}
}
