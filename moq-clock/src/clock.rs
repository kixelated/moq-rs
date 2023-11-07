use std::time;

use anyhow::Context;
use moq_transport::{
	cache::{fragment, segment, track},
	VarInt,
};

use chrono::prelude::*;

pub struct Publisher {
	track: track::Publisher,
}

impl Publisher {
	pub fn new(track: track::Publisher) -> Self {
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
				.create_segment(segment::Info {
					sequence: VarInt::from_u32(sequence),
					priority: 0,
					expires: Some(time::Duration::from_secs(60)),
				})
				.context("failed to create minute segment")?;

			sequence += 1;

			tokio::spawn(async move {
				if let Err(err) = Self::send_segment(segment, now).await {
					log::warn!("failed to send minute: {:?}", err);
				}
			});

			let next = now + chrono::Duration::minutes(1);
			let next = next.with_second(0).unwrap().with_nanosecond(0).unwrap();

			let delay = (next - now).to_std().unwrap();
			tokio::time::sleep(delay).await;

			now = next; // just assume we didn't undersleep
		}
	}

	async fn send_segment(mut segment: segment::Publisher, mut now: DateTime<Utc>) -> anyhow::Result<()> {
		// Everything but the second.
		let base = now.format("%Y-%m-%d %H:%M:").to_string();

		segment
			.fragment(VarInt::ZERO, base.len())?
			.chunk(base.clone().into())
			.context("failed to write base")?;

		loop {
			let delta = now.format("%S").to_string();
			let sequence = VarInt::from_u32(now.second() + 1);

			segment
				.fragment(sequence, delta.len())?
				.chunk(delta.clone().into())
				.context("failed to write delta")?;

			println!("{}{}", base, delta);

			let next = now + chrono::Duration::seconds(1);
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
	track: track::Subscriber,
}

impl Subscriber {
	pub fn new(track: track::Subscriber) -> Self {
		Self { track }
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		while let Some(segment) = self.track.segment().await.context("failed to get segment")? {
			log::debug!("got segment: {:?}", segment);
			tokio::spawn(async move {
				if let Err(err) = Self::recv_segment(segment).await {
					log::warn!("failed to receive segment: {:?}", err);
				}
			});
		}

		Ok(())
	}

	async fn recv_segment(mut segment: segment::Subscriber) -> anyhow::Result<()> {
		let first = segment
			.fragment()
			.await
			.context("failed to get first fragment")?
			.context("no fragments in segment")?;

		log::debug!("got first: {:?}", first);

		if first.sequence.into_inner() != 0 {
			anyhow::bail!("first object must be zero; I'm not going to implement a reassembly buffer");
		}

		let base = Self::recv_fragment(first, Vec::new()).await?;

		log::debug!("read base: {:?}", String::from_utf8_lossy(&base));

		while let Some(fragment) = segment.fragment().await? {
			log::debug!("next fragment: {:?}", fragment);
			let value = Self::recv_fragment(fragment, base.clone()).await?;
			let str = String::from_utf8(value).context("invalid UTF-8")?;

			println!("{}", str);
		}

		Ok(())
	}

	async fn recv_fragment(mut fragment: fragment::Subscriber, mut buf: Vec<u8>) -> anyhow::Result<Vec<u8>> {
		while let Some(data) = fragment.chunk().await? {
			buf.extend_from_slice(&data);
		}

		Ok(buf)
	}
}
