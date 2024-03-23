use std::{
	io::{Cursor, Write},
	sync::Arc,
};

use anyhow::Context;
use log::{debug, info, trace, warn};
use moq_transport::cache::{broadcast, fragment, segment, track};
use mp4::ReadBox;
use tokio::{
	io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
	sync::Mutex,
	task::JoinSet,
};

pub struct Media<O> {
	broadcast: broadcast::Subscriber,
	output: Arc<Mutex<O>>,
}

impl<O: AsyncWrite + Send + Unpin + 'static> Media<O> {
	pub async fn new(broadcast: broadcast::Subscriber, output: O) -> anyhow::Result<Self> {
		Ok(Self {
			broadcast,
			output: Arc::new(Mutex::new(output)),
		})
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		let moov = {
			let init_track_name = "0.mp4";
			let mut track = self.broadcast.get_track(&init_track_name)?;
			let mut segment = track.segment().await?.context("no init segment")?;
			let fragment = segment.fragment().await?.context("no init fragment")?;
			let buf = Self::recv_fragment(fragment).await?;
			self.output.lock().await.write_all(&buf).await?;
			let mut reader = Cursor::new(&buf);

			let ftyp = read_atom(&mut reader).await?;
			anyhow::ensure!(&ftyp[4..8] == b"ftyp", "expected ftyp atom");

			let moov = read_atom(&mut reader).await?;
			anyhow::ensure!(&moov[4..8] == b"moov", "expected moov atom");
			let mut moov_reader = Cursor::new(&moov);
			let moov_header = mp4::BoxHeader::read(&mut moov_reader)?;
			let moov = mp4::MoovBox::read_box(&mut moov_reader, moov_header.size)?;
			moov
		};

		let mut has_video = false;
		let mut has_audio = false;
		let mut tracks = vec![];
		for trak in &moov.traks {
			let id = trak.tkhd.track_id;
			let name = format!("{}.m4s", id);
			info!("found track {name}");
			let mut active = false;
			if !has_video && trak.mdia.minf.stbl.stsd.avc1.is_some() {
				active = true;
				has_video = true;
				info!("using {name} for video");
			}
			if !has_audio && trak.mdia.minf.stbl.stsd.mp4a.is_some() {
				active = true;
				has_audio = true;
				info!("using {name} for audio");
			}
			if active {
				tracks.push(self.broadcast.get_track(&name)?);
			}
		}

		info!("playing {} tracks", tracks.len());
		let mut tasks = JoinSet::new();
		for track in tracks {
			let out = self.output.clone();
			tasks.spawn(async move {
				let name = track.name.clone();
				if let Err(err) = Self::recv_track(track, out).await {
					warn!("failed to play track {name}: {err:?}");
				}
			});
		}
		while let Some(_) = tasks.join_next().await {}
		Ok(())
	}

	async fn recv_track(mut track: track::Subscriber, out: Arc<Mutex<O>>) -> anyhow::Result<()> {
		let name = track.name.clone();
		debug!("track {name}: start");
		while let Some(segment) = track.segment().await? {
			let out = out.clone();
			tokio::task::spawn(async move {
				if let Err(err) = Self::recv_segment(segment, out).await {
					warn!("Failed to receive segment: {err:?}");
				}
			});
		}
		debug!("track {name}: finish");
		Ok(())
	}

	async fn recv_segment(mut segment: segment::Subscriber, out: Arc<Mutex<O>>) -> anyhow::Result<()> {
		trace!("segment={} start", segment.sequence);
		while let Some(fragment) = segment.fragment().await? {
			trace!("segment={} fragment={}", segment.sequence, fragment.sequence);
			let buf = Self::recv_fragment(fragment).await?;
			out.lock().await.write_all(&buf).await?;
		}
		Ok(())
	}

	async fn recv_fragment(mut fragment: fragment::Subscriber) -> anyhow::Result<Vec<u8>> {
		let mut buf = match fragment.size {
			Some(cap) => Vec::with_capacity(cap),
			None => Vec::new(),
		};
		while let Some(chunk) = fragment.chunk().await? {
			buf.extend_from_slice(&chunk);
		}
		Ok(buf)
	}
}

// Read a full MP4 atom into a vector.
async fn read_atom<R: AsyncReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Vec<u8>> {
	// Read the 8 bytes for the size + type
	let mut buf = [0u8; 8];
	reader.read_exact(&mut buf).await?;

	// Convert the first 4 bytes into the size.
	let size = u32::from_be_bytes(buf[0..4].try_into()?) as u64;

	let mut raw = buf.to_vec();

	let mut limit = match size {
		// Runs until the end of the file.
		0 => reader.take(u64::MAX),

		// The next 8 bytes are the extended size to be used instead.
		1 => {
			reader.read_exact(&mut buf).await?;
			let size_large = u64::from_be_bytes(buf);
			anyhow::ensure!(size_large >= 16, "impossible extended box size: {}", size_large);

			reader.take(size_large - 16)
		}

		2..=7 => {
			anyhow::bail!("impossible box size: {}", size)
		}

		size => reader.take(size - 8),
	};

	// Append to the vector and return it.
	let _read_bytes = limit.read_to_end(&mut raw).await?;

	Ok(raw)
}
