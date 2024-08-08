use std::time;

use super::{Result, Root};

pub struct Producer {
	track: moq_transfork::TrackProducer,
}

impl Producer {
	pub fn new(track: moq_transfork::TrackProducer) -> Self {
		Self { track }
	}

	pub fn publish(broadcast: &mut moq_transfork::BroadcastProducer) -> Result<Self> {
		let track = moq_transfork::Track::build("catalog.json", 0)
			.group_order(moq_transfork::GroupOrder::Descending)
			.group_expires(time::Duration::ZERO)
			.into();
		let track = broadcast.insert_track(track);
		Ok(Self::new(track))
	}

	pub fn write(&mut self, root: Root) -> Result<()> {
		let frame = root.to_string()?;
		let mut group = self.track.append_group();
		group.write_frame(frame.into());

		Ok(())
	}
}
