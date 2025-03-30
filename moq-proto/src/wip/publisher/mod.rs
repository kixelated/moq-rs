mod announce;
mod group;
mod subscribe;

pub use announce::*;
pub use group::*;
pub use subscribe::*;

#[derive(Default)]
pub struct Publisher {
	announces: PublisherAnnounces,
	subscribes: PublisherSubscribes,
	groups: PublisherGroups,
}

impl Publisher {
	pub fn announces(&mut self) -> &mut PublisherAnnounces {
		&mut self.announces
	}

	pub fn subscribes(&mut self) -> &mut PublisherSubscribes {
		&mut self.subscribes
	}

	pub fn groups(&mut self) -> &mut PublisherGroups {
		&mut self.groups
	}
}
