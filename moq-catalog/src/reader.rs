use std::time;

use crate::{Error, Result, Root};

pub struct Reader {
	track: moq_transfork::TrackReader,
}

impl Reader {
	pub fn new(track: moq_transfork::TrackReader) -> Self {
		Self { track }
	}

	pub async fn subscribe(broadcast: &moq_transfork::BroadcastReader) -> Result<Self> {
		let track = moq_transfork::Track::new("catalog.json", 0)
			.group_order(moq_transfork::GroupOrder::Descending)
			.group_expires(time::Duration::ZERO)
			.build();
		let track = broadcast.subscribe(track).await?;
		Ok(Self::new(track))
	}

	pub async fn read(&mut self) -> Result<Root> {
		let mut group = self.track.next_group().await?.ok_or(Error::Empty)?;
		let frame = group.read_frame().await?.ok_or(Error::Empty)?;
		tracing::debug!(raw = %String::from_utf8_lossy(&frame), "decoding catalog");
		Root::from_slice(&frame)
	}

	// TODO support updates
}
