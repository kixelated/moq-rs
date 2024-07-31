use anyhow::Context;
use moq_transfork::prelude::*;

use futures::{stream::FuturesUnordered, StreamExt};

use bytes::Bytes;

pub struct Media {
	// The init segment for the media
	init: Option<Bytes>,

	// The tracks that the media is composed of
	tracks: Vec<MediaTrack>,
}

impl Media {
	pub async fn load(broadcast: &BroadcastReader) -> anyhow::Result<Self> {
		let catalog = moq_catalog::Reader::subscribe(&broadcast).await?.read().await?;
		let mut tracks = Vec::new();

		let init = Self::load_init(&catalog, &broadcast).await?;

		for track in catalog.tracks {
			// TODO proper track typing
			let priority = match track.selection_params.width {
				Some(_) => 2,
				_ => 1,
			};

			let track = Track::new(track.name, priority).build();
			let track = broadcast.subscribe(track).await?;
			let track = MediaTrack::new(track);
			tracks.push(track);
		}

		Ok(Self { init, tracks })
	}

	// TODO This is quite limited because we can currently only flush a single fMP4 init header
	async fn load_init(catalog: &moq_catalog::Root, broadcast: &BroadcastReader) -> anyhow::Result<Option<Bytes>> {
		for track in &catalog.tracks {
			if let Some(name) = &track.init_track {
				let track = moq_transfork::Track::new(name, 0).build();
				let mut track = broadcast.subscribe(track).await?;

				let mut group = track.next_group().await?.context("no groups")?;
				let frame = group.read_frame().await?.context("no frames")?;

				return Ok(Some(frame));
			}
		}

		Ok(None)
	}

	pub fn init(&self) -> Option<&Bytes> {
		self.init.as_ref()
	}

	// Returns the next atom in any track
	pub async fn next(&mut self) -> anyhow::Result<Option<Bytes>> {
		let mut futures = FuturesUnordered::new();

		for track in &mut self.tracks {
			futures.push(track.next());
		}

		loop {
			match futures.next().await {
				Some(Err(err)) => return Err(err),
				Some(Ok(Some(next))) => return Ok(Some(next)),
				Some(Ok(None)) => continue,
				None => return Ok(None),
			};
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
	pub async fn next(&mut self) -> anyhow::Result<Option<Bytes>> {
		if self.current.is_none() {
			self.current = self.groups.next_group().await?;
		}

		let mut track_eof = false;
		let mut group_eof = false;

		loop {
			tokio::select! {
				res = self.groups.next_group(), if !track_eof => {
					if let Some(group) = res? {
						// TODO only drop the current group after a configurable latency
						self.current.replace(group);
						group_eof = false;
					} else {
						track_eof = true;
					}
				}
				res = self.current.as_mut().unwrap().read_frame(), if !group_eof => {
					if let Some(frame) = res? {
						return Ok(Some(frame));
					} else {
						group_eof = true;
					}
				}
			}
		}
	}
}
