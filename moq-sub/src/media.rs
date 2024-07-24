use std::{future::Future, io::Cursor, pin::Pin};

use anyhow::Context;
use moq_transfork::prelude::*;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use mp4::ReadBox;

use bytes::Bytes;

pub struct Media {
	// The init segment for the media
	init: Option<Bytes>,

	// Returns the next atom for an track, and the track so it can be polled again
	tasks: FuturesUnordered<Pin<Box<dyn Future<Output = anyhow::Result<(MediaTrack, Option<Bytes>)>> + Send>>>,
}

impl Media {
	// TODO TODO use the catalog to discover tracks, not the init segment
	pub async fn load(mut subscriber: Subscriber, broadcast: Broadcast) -> anyhow::Result<Self> {
		let init = Track::new("0.mp4", 0).build();
		let mut init = subscriber.subscribe(broadcast.clone(), init).await?;

		let init = init
			.next()
			.await?
			.context("empty init track")?
			.read()
			.await?
			.context("empty init group")?;

		let ftyp = next_atom(&init)?;
		anyhow::ensure!(&ftyp[4..8] == b"ftyp", "expected ftyp atom");

		let moov = next_atom(&init[ftyp.len()..])?;
		anyhow::ensure!(&moov[4..8] == b"moov", "expected moov atom");

		let mut moov_reader = Cursor::new(&moov);
		let moov_header = mp4::BoxHeader::read(&mut moov_reader)?;

		let moov = mp4::MoovBox::read_box(&mut moov_reader, moov_header.size)?;

		let mut has_video = false;
		let mut has_audio = false;

		let tasks = FuturesUnordered::new();

		for trak in &moov.traks {
			let id = trak.tkhd.track_id;
			let name = format!("{}.m4s", id);
			log::info!("found track {name}");

			let mut active = false;
			if !has_video && trak.mdia.minf.stbl.stsd.avc1.is_some() {
				active = true;
				has_video = true;
				log::info!("using {name} for video");
			}
			if !has_audio && trak.mdia.minf.stbl.stsd.mp4a.is_some() {
				active = true;
				has_audio = true;
				log::info!("using {name} for audio");
			}

			if active {
				let track_type =
					mp4::TrackType::try_from(&trak.mdia.hdlr.handler_type).context("unnknown track type")?;

				let priority = match track_type {
					mp4::TrackType::Video => 3,
					mp4::TrackType::Audio => 2,
					mp4::TrackType::Subtitle => 1,
				};

				let track = Track::new(name, priority).build();

				let track = subscriber.subscribe(broadcast.clone(), track).await?;
				let track = MediaTrack::new(track);
				tasks.push(track.next().boxed());
			}
		}

		Ok(Self {
			init: Some(init),
			tasks,
		})
	}

	// Returns the next atom in any track
	pub async fn next(&mut self) -> anyhow::Result<Option<Bytes>> {
		if let Some(init) = self.init.take() {
			return Ok(Some(init));
		}

		loop {
			if let Some(res) = self.tasks.next().await {
				if let (track, Some(next)) = res? {
					self.tasks.push(track.next().boxed());
					return Ok(Some(next));
				}
			} else {
				return Ok(None);
			}
		}
	}
}

struct MediaTrack {
	groups: TrackReader,
	current: Option<GroupReader>,
}

impl MediaTrack {
	pub fn new(track: TrackReader) -> Self {
		Self {
			groups: track,
			current: None,
		}
	}

	// Returns the next atom in the current track
	pub async fn next(mut self) -> anyhow::Result<(Self, Option<Bytes>)> {
		if self.current.is_none() {
			self.current = self.groups.next().await?;
		}

		let mut track_eof = false;
		let mut group_eof = false;

		loop {
			tokio::select! {
				res = self.groups.next(), if !track_eof => {
					if let Some(group) = res? {
						// TODO only drop the current group after a configurable latency
						self.current.replace(group);
						group_eof = false;
					} else {
						track_eof = true;
					}
				}
				res = self.current.as_mut().unwrap().read(), if !group_eof => {
					if let Some(frame) = res? {
						return Ok((self, Some(frame)));
					} else {
						group_eof = true;
					}
				}
			}
		}
	}
}

// Returns the next atom in the buffer.
fn next_atom<'a>(buf: &'a [u8]) -> anyhow::Result<&'a [u8]> {
	// Convert the first 4 bytes into the size.
	let size = u32::from_be_bytes(buf[0..4].try_into()?) as usize;

	Ok(match size {
		// Until the end of the atom
		0 => buf,

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			let size = u64::from_be_bytes(buf[8..16].try_into()?) as usize;
			anyhow::ensure!(size >= 16, "impossible extended box size: {}", size);
			&buf[..size]
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
		}

		size => &buf[..size],
	})
}
