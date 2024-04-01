use anyhow::Context;
use moq_transport::serve::{
	DatagramsReader, Group, GroupWriter, GroupsReader, GroupsWriter, ObjectsReader, StreamReader, TrackReader,
	TrackReaderMode,
};

use chrono::prelude::*;

pub struct Publisher {
	track: GroupsWriter,
}

impl Publisher {
	pub fn new(track: GroupsWriter) -> Self {
		Self { track }
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		let start = Utc::now();
		let mut now = start;

		// Just for fun, don't start at zero.
		let mut sequence = start.minute();

		loop {
			let segment = self
				.track
				.create(Group {
					group_id: sequence as u64,
					priority: 0,
				})
				.context("failed to create minute segment")?;

			sequence += 1;

			tokio::spawn(async move {
				if let Err(err) = Self::send_segment(segment, now).await {
					log::warn!("failed to send minute: {:?}", err);
				}
			});

			let next = now + chrono::Duration::try_minutes(1).unwrap();
			let next = next.with_second(0).unwrap().with_nanosecond(0).unwrap();

			let delay = (next - now).to_std().unwrap();
			tokio::time::sleep(delay).await;

			now = next; // just assume we didn't undersleep
		}
	}

	async fn send_segment(mut segment: GroupWriter, mut now: DateTime<Utc>) -> anyhow::Result<()> {
		// Everything but the second.
		let base = now.format("%Y-%m-%d %H:%M:").to_string();

		segment.write(base.clone().into()).context("failed to write base")?;

		loop {
			let delta = now.format("%S").to_string();
			segment.write(delta.clone().into()).context("failed to write delta")?;

			println!("{}{}", base, delta);

			let next = now + chrono::Duration::try_seconds(1).unwrap();
			let next = next.with_nanosecond(0).unwrap();

			let delay = (next - now).to_std().unwrap();
			tokio::time::sleep(delay).await;

			// Get the current time again to check if we overslept
			let next = Utc::now();
			if next.minute() != now.minute() {
				return Ok(());
			}

			now = next;
		}
	}
}
pub struct Subscriber {
	track: TrackReader,
}

impl Subscriber {
	pub fn new(track: TrackReader) -> Self {
		Self { track }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		match self.track.mode().await.context("failed to get mode")? {
			TrackReaderMode::Stream(stream) => Self::recv_stream(stream).await,
			TrackReaderMode::Groups(groups) => Self::recv_groups(groups).await,
			TrackReaderMode::Objects(objects) => Self::recv_objects(objects).await,
			TrackReaderMode::Datagrams(datagrams) => Self::recv_datagrams(datagrams).await,
		}
	}

	async fn recv_stream(mut track: StreamReader) -> anyhow::Result<()> {
		while let Some(mut group) = track.next().await? {
			while let Some(object) = group.read_next().await? {
				let str = String::from_utf8_lossy(&object);
				println!("{}", str);
			}
		}

		Ok(())
	}

	async fn recv_groups(mut groups: GroupsReader) -> anyhow::Result<()> {
		while let Some(mut group) = groups.next().await? {
			let base = group
				.read_next()
				.await
				.context("failed to get first object")?
				.context("empty group")?;

			let base = String::from_utf8_lossy(&base);

			while let Some(object) = group.read_next().await? {
				let str = String::from_utf8_lossy(&object);
				println!("{}{}", base, str);
			}
		}

		Ok(())
	}

	async fn recv_objects(mut objects: ObjectsReader) -> anyhow::Result<()> {
		while let Some(mut object) = objects.next().await? {
			let payload = object.read_all().await?;
			let str = String::from_utf8_lossy(&payload);
			println!("{}", str);
		}

		Ok(())
	}

	async fn recv_datagrams(mut datagrams: DatagramsReader) -> anyhow::Result<()> {
		while let Some(datagram) = datagrams.read().await? {
			let str = String::from_utf8_lossy(&datagram.payload);
			println!("{}", str);
		}

		Ok(())
	}
}
