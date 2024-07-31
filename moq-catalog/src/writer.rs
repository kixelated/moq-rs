use std::time;

use crate::{Result, Root};

pub struct Writer {
	track: moq_transfork::TrackWriter,
}

impl Writer {
	pub fn new(track: moq_transfork::TrackWriter) -> Self {
		Self { track }
	}

	pub fn publish(broadcast: &mut moq_transfork::BroadcastWriter) -> Result<Self> {
		let track = moq_transfork::Track::new("catalog.json", 0)
			.group_order(moq_transfork::GroupOrder::Descending)
			.group_expires(time::Duration::ZERO)
			.build();
		let track = broadcast.insert_track(track)?;
		Ok(Self::new(track))
	}

	pub fn write(&mut self, root: Root) -> Result<()> {
		let frame = root.to_string()?;
		tracing::debug!(raw = frame, "encoded catalog");

		let mut group = self.track.append_group()?;
		group.write_frame(frame.into())?;

		Ok(())
	}
}
