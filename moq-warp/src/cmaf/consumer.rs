use moq_transfork::prelude::*;

use futures::{stream::FuturesUnordered, StreamExt};

use bytes::Bytes;

use super::Error;
use crate::catalog;

pub struct Consumer {
	// The init segment for the media
	init: Option<Bytes>,

	// The tracks that the media is composed of
	tracks: Vec<MediaTrack>,
}

impl Consumer {
	pub async fn load(broadcast: BroadcastReader) -> Result<Self, Error> {
		let catalog = catalog::Reader::subscribe(&broadcast).await?.read().await?;
		tracing::info!(?catalog);

		let mut tracks = Vec::new();

		let init = Self::load_init(&catalog, &broadcast).await?;

		for track in catalog.tracks {
			// TODO proper track typing
			let priority = match track.selection_params.width {
				Some(_) => 2,
				_ => 1,
			};

			let track = Track::create(track.name, priority).build();
			let track = broadcast.subscribe(track).await?;
			let track = MediaTrack::new(track);
			tracks.push(track);
		}

		Ok(Self { init, tracks })
	}

	// TODO This is quite limited because we can currently only flush a single fMP4 init header
	async fn load_init(catalog: &catalog::Root, broadcast: &BroadcastReader) -> Result<Option<Bytes>, Error> {
		for track in &catalog.tracks {
			if let Some(name) = &track.init_track {
				let track = moq_transfork::Track::create(name, 0).build();
				let mut track = broadcast.subscribe(track).await?;

				let mut group = track.next_group().await?.ok_or(Error::EmptyInit)?;
				let frame = group.read_frame().await?.ok_or(Error::EmptyInit)?;

				return Ok(Some(frame));
			}
		}

		Ok(None)
	}

	pub fn init(&self) -> Option<&Bytes> {
		self.init.as_ref()
	}

	// Returns the next atom in any track
	pub async fn next(&mut self) -> Result<Option<Bytes>, Error> {
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
	pub async fn next(&mut self) -> Result<Option<Bytes>, Error> {
		loop {
			match self.current.as_mut() {
				Some(group) => {
					if let Some(frame) = group.read_frame().await? {
						return Ok(Some(frame));
					} else {
						self.current = None;
					}
				}
				None => self.current = self.groups.next_group().await?,
			}
		}
	}
}
