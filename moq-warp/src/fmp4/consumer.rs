use bytes::{Buf, Bytes};
use mp4::ReadBox;

use super::{Error, Frame, Init, Timestamp};
use crate::catalog;
use crate::util::FuturesExt;

#[derive(Clone)]
pub struct BroadcastConsumer {
	pub catalog: catalog::Broadcast,
	pub init: Init,

	inner: moq_transfork::BroadcastConsumer,
}

impl BroadcastConsumer {
	pub async fn load(broadcast: moq_transfork::BroadcastConsumer) -> Result<Self, Error> {
		let catalog = catalog::Broadcast::fetch(broadcast.clone()).await?;

		let init = catalog.init.get(&catalog::Container::Fmp4).ok_or(Error::MissingInit)?;
		let init = Self::parse_init(Bytes::copy_from_slice(init))?;

		Ok(Self {
			inner: broadcast,
			catalog,
			init,
		})
	}

	fn parse_init(raw: Bytes) -> Result<Init, Error> {
		let mut reader = std::io::Cursor::new(&raw);

		let styp = mp4::BoxHeader::read(&mut reader)?;
		if styp.name != mp4::BoxType::UnknownBox("styp".as_bytes().get_u32()) {
			return Err(Error::ExpectedBox("styp"));
		}

		let styp = mp4::FtypBox::read_box(&mut reader, styp.size)?;

		let moov = mp4::BoxHeader::read(&mut reader)?;
		if moov.name != mp4::BoxType::MoovBox {
			return Err(Error::ExpectedBox("moov"));
		}

		let moov = mp4::MoovBox::read_box(&mut reader, moov.size)?;

		if reader.remaining() > 0 {
			return Err(Error::TrailingData);
		}

		let init = Init { styp, moov, raw };

		Ok(init)
	}

	pub async fn subscribe<T: Into<moq_transfork::Track>>(&self, track: T) -> Result<TrackConsumer, Error> {
		let track = self.inner.subscribe(track).await?;
		Ok(TrackConsumer::new(track, self.init.clone()))
	}
}

pub struct TrackConsumer {
	pub init: Init,

	track: moq_transfork::TrackConsumer,
	group: Option<moq_transfork::GroupConsumer>,
	keyframe: bool,
}

impl TrackConsumer {
	pub fn new(track: moq_transfork::TrackConsumer, init: Init) -> Self {
		Self {
			track,
			init,
			group: None,
			keyframe: false,
		}
	}

	pub async fn read(&mut self) -> Result<Option<Frame>, Error> {
		if self.group.is_none() {
			self.group = self.track.next_group().await?;
			self.keyframe = true;

			if self.group.is_none() {
				return Ok(None);
			}
		}

		loop {
			tokio::select! {
				biased;
				Some(frame) = self.group.as_mut().unwrap().read_frame().transpose() => {
					let frame = self.parse_frame(frame?)?;
					return Ok(Some(frame));
				},
				Some(group) = self.track.next_group().transpose() => {
					let group = group?;

					if group.sequence < self.group.as_ref().unwrap().sequence {
						// Ignore old groups
						continue;
					}

					// TODO use a configurable latency before moving to the next group.
					self.group = Some(group);
					self.keyframe = true;
				},
				else => return Ok(None),
			}
		}
	}

	fn parse_frame(&mut self, raw: Bytes) -> Result<Frame, Error> {
		let mut reader = std::io::Cursor::new(&raw);

		let moof = mp4::BoxHeader::read(&mut reader)?;
		if moof.name != mp4::BoxType::MoofBox {
			return Err(Error::ExpectedBox("moof"));
		}

		let moof = mp4::MoofBox::read_box(&mut reader, moof.size)?;
		let timestamp = self.parse_timestamp(&moof)?;

		let mdat = mp4::BoxHeader::read(&mut reader)?;
		if mdat.name != mp4::BoxType::MdatBox {
			return Err(Error::ExpectedBox("mdat"));
		}

		let start = reader.position() as usize;
		let end = start + mdat.size as usize;

		if end != raw.len() {
			return Err(Error::TrailingData);
		}

		let payload = raw.slice(start..end);

		let frame = Frame {
			timestamp,
			keyframe: self.keyframe,
			payload,
			raw,
		};

		self.keyframe = false;

		Ok(frame)
	}

	fn parse_timestamp(&self, moof: &mp4::MoofBox) -> Result<Timestamp, Error> {
		let traf = match moof.trafs[..] {
			[ref traf] => traf,
			[] => return Err(Error::MissingBox("traf")),
			_ => return Err(Error::DuplicateBox("traf")),
		};

		let tfdt = traf.tfdt.as_ref().ok_or(Error::MissingBox("tfdt"))?;
		let base = tfdt.base_media_decode_time;

		let trak = self
			.init
			.moov
			.traks
			.iter()
			.find(|trak| trak.tkhd.track_id == traf.tfhd.track_id)
			.ok_or(Error::MissingBox("trak"))?;

		let scale = trak.mdia.mdhd.timescale as u64;

		let timestamp = Timestamp { base, scale };
		Ok(timestamp)
	}
}
