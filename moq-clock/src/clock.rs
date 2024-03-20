use anyhow::Context;
use moq_transport::serve;

use chrono::prelude::*;

pub struct Publisher {
	track: serve::TrackPublisher,
}

impl Publisher {
	pub fn new(track: serve::TrackPublisher) -> Self {
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
				.create_group(serve::Group {
					id: sequence as u64,
					send_order: 0,
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

	async fn send_segment(mut segment: serve::GroupPublisher, mut now: DateTime<Utc>) -> anyhow::Result<()> {
		// Everything but the second.
		let base = now.format("%Y-%m-%d %H:%M:").to_string();

		segment
			.write_object(base.clone().into())
			.context("failed to write base")?;

		loop {
			let delta = now.format("%S").to_string();
			segment
				.write_object(delta.clone().into())
				.context("failed to write delta")?;

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
	track: serve::TrackSubscriber,
}

impl Subscriber {
	pub fn new(track: serve::TrackSubscriber) -> Self {
		Self { track }
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		while let Some(stream) = self.track.next().await.context("failed to get stream")? {
			match stream {
				serve::TrackMode::Group(group) => tokio::spawn(async move {
					if let Err(err) = Self::recv_group(group).await {
						log::warn!("failed to receive group: {:?}", err);
					}
				}),
				serve::TrackMode::Object(object) => tokio::spawn(async move {
					if let Err(err) = Self::recv_object(object).await {
						log::warn!("failed to receive group: {:?}", err);
					}
				}),
				serve::TrackMode::Stream(stream) => tokio::spawn(async move {
					if let Err(err) = Self::recv_track(stream).await {
						log::warn!("failed to receive stream: {:?}", err);
					}
				}),
				serve::TrackMode::Datagram(datagram) => tokio::spawn(async move {
					if let Err(err) = Self::recv_datagram(datagram) {
						log::warn!("failed to receive datagram: {:?}", err);
					}
				}),
			};
		}

		Ok(())
	}

	async fn recv_track(mut track: serve::StreamSubscriber) -> anyhow::Result<()> {
		while let Some(fragment) = track.next().await? {
			let str = String::from_utf8_lossy(&fragment.payload);
			println!("{}", str);
		}

		Ok(())
	}

	async fn recv_group(mut segment: serve::GroupSubscriber) -> anyhow::Result<()> {
		let mut first = segment
			.next()
			.await
			.context("failed to get first fragment")?
			.context("no fragments in segment")?;

		let base = first.read_all().await?;
		let base = String::from_utf8_lossy(&base);

		while let Some(mut fragment) = segment.next().await? {
			let value = fragment.read_all().await.context("failed to read fragment")?;
			let str = String::from_utf8_lossy(&value);

			println!("{}{}", base, str);
		}

		Ok(())
	}

	async fn recv_object(mut object: serve::ObjectSubscriber) -> anyhow::Result<()> {
		let value = object.read_all().await.context("failed to read object")?;
		let str = String::from_utf8_lossy(&value);

		println!("{}", str);
		Ok(())
	}

	fn recv_datagram(datagram: serve::Datagram) -> anyhow::Result<()> {
		let str = String::from_utf8_lossy(&datagram.payload);
		println!("{}", str);
		Ok(())
	}
}
