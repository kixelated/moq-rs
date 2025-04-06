use derive_more::{From, Into};

macro_rules! create_id {
	($($name:ident),*,) => {
		$(
			#[derive(From, Into, Debug, Default, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
			pub struct $name(pub u64);

			impl $name {
				pub fn increment(&mut self) {
					self.0 += 1;
				}
			}
		)*
	}
}

// Create a wrapper around a u64 for more type safety.
// You can get the underlying u64 via From/Into.
create_id! {
	AnnounceId,
	SubscribeId,
	GroupId,
	StreamId,
}
