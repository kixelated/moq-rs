use bytes::BytesMut;
use mp4_atom::{Any, AsyncReadFrom, Atom, DecodeMaybe, Esds, Moof, Moov, Trak};
use std::collections::HashMap;
use tokio::io::AsyncRead;

use super::{util, Error, Result};
use crate::{catalog, media};

/// Converts fMP4 -> Karp
pub struct Import {
	// Any partial data in the input buffer
	buffer: BytesMut,

	// The broadcast being produced
	broadcast: media::BroadcastProducer,

	// A lookup to tracks in the broadcast
	tracks: HashMap<u32, media::TrackProducer>,

	// The moov atom at the start of the file.
	moov: Option<Moov>,

	// The latest moof header
	moof: Option<Moof>,
}

impl Import {
	pub fn new(broadcast: moq_transfork::BroadcastProducer) -> Self {
		Self {
			buffer: BytesMut::new(),
			broadcast: media::BroadcastProducer::new(broadcast),
			tracks: HashMap::default(),
			moov: None,
			moof: None,
		}
	}

	pub fn parse(&mut self, data: &[u8]) -> Result<()> {
		if self.broadcast.is_closed() {
			return Err(Error::Closed);
		}

		if !self.buffer.is_empty() {
			let mut buffer = std::mem::replace(&mut self.buffer, BytesMut::new());
			buffer.extend_from_slice(data);
			let n = self.parse_inner(&buffer)?;
			self.buffer = buffer.split_off(n);
		} else {
			let n = self.parse_inner(data)?;
			self.buffer = data[n..].into();
		}

		Ok(())
	}

	fn parse_inner<T: AsRef<[u8]>>(&mut self, data: T) -> Result<usize> {
		let mut cursor = std::io::Cursor::new(data);

		while let Some(atom) = mp4_atom::Any::decode_maybe(&mut cursor)? {
			self.process(atom)?;
		}

		// Return the number of bytes consumed
		Ok(cursor.position() as usize)
	}

	fn init(&mut self, moov: Moov) -> Result<()> {
		// Produce the catalog
		for trak in &moov.trak {
			let track_id = trak.tkhd.track_id;
			let handler = &trak.mdia.hdlr.handler;

			let track = match handler.as_ref() {
				b"vide" => {
					let track = Self::init_video(trak)?;
					self.broadcast.create_video(track)?
				}
				b"soun" => {
					let track = Self::init_audio(trak)?;
					self.broadcast.create_audio(track)?
				}
				b"sbtl" => return Err(Error::UnsupportedTrack("subtitle")),
				_ => return Err(Error::UnsupportedTrack("unknown")),
			};

			self.tracks.insert(track_id, track);
		}

		self.broadcast.publish()?;
		self.moov = Some(moov);

		Ok(())
	}

	fn init_video(trak: &Trak) -> Result<catalog::Video> {
		let name = trak.tkhd.track_id.to_string();
		let stsd = &trak.mdia.minf.stbl.stsd;
		let timescale = trak.mdia.mdhd.timescale;

		let track = if let Some(avc1) = &stsd.avc1 {
			let avcc = &avc1.avcc;

			let mut description = BytesMut::new();
			avcc.encode_body(&mut description)?;

			catalog::Video {
				track: moq_transfork::Track::build(name).priority(2).into(),
				resolution: catalog::Dimensions {
					width: avc1.width,
					height: avc1.height,
				},
				codec: catalog::H264 {
					profile: avcc.avc_profile_indication,
					constraints: avcc.profile_compatibility,
					level: avcc.avc_level_indication,
				}
				.into(),
				description: description.freeze(),
				timescale,
				layers: vec![],
				bitrate: None,
			}
		} else if let Some(hev1) = &stsd.hev1 {
			let _hvcc = &hev1.hvcc;

			/*
			catalog::Video {
				track: moq_transfork::Track::build(name).priority(2).into(),
				width: hev1.width,
				height: hev1.height,
				codec: catalog::H265 {
					profile: hvcc.general_profile_idc,
					level: hvcc.general_level_idc,
					constraints: hvcc.general_constraint_indicator_flag,
				},
				container: catalog::Container::Fmp4,
				layers: vec![],
				bit_rate: None,
			}
			*/

			// Just waiting for a release:
			// https://github.com/alfg/mp4-rust/commit/35560e94f5e871a2b2d88bfe964013b39af131e8
			return Err(Error::UnsupportedCodec("HEVC"));
		} else if let Some(vp09) = &stsd.vp09 {
			// https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L238
			let vpcc = &vp09.vpcc;

			catalog::Video {
				track: moq_transfork::Track::build(name).priority(2).into(),
				codec: catalog::VP9 {
					profile: vpcc.profile,
					level: vpcc.level,
					bit_depth: vpcc.bit_depth,
					chroma_subsampling: vpcc.chroma_subsampling,
					color_primaries: vpcc.color_primaries,
					transfer_characteristics: vpcc.transfer_characteristics,
					matrix_coefficients: vpcc.matrix_coefficients,
					full_range: vpcc.video_full_range_flag,
				}
				.into(),
				timescale,
				description: Default::default(),
				resolution: catalog::Dimensions {
					width: vp09.width,
					height: vp09.height,
				},
				layers: vec![],
				bitrate: None,
			}
		} else {
			// TODO add av01 support: https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L251
			return Err(Error::UnsupportedCodec("unknown"));
		};

		Ok(track)
	}

	fn init_audio(trak: &Trak) -> Result<catalog::Audio> {
		let name = trak.tkhd.track_id.to_string();
		let stsd = &trak.mdia.minf.stbl.stsd;
		let timescale = trak.mdia.mdhd.timescale;

		let track = if let Some(mp4a) = &stsd.mp4a {
			let desc = &mp4a
				.esds
				.as_ref()
				.ok_or(Error::MissingBox(Esds::KIND))?
				.es_desc
				.dec_config;

			// TODO Also support mp4a.67
			if desc.object_type_indication != 0x40 {
				return Err(Error::UnsupportedCodec("MPEG2"));
			}

			catalog::Audio {
				track: moq_transfork::Track::build(name).priority(1).into(),
				codec: catalog::AAC {
					profile: desc.dec_specific.profile,
				}
				.into(),
				timescale,
				sample_rate: mp4a.samplerate.integer(),
				channel_count: mp4a.channelcount,
				bitrate: Some(std::cmp::max(desc.avg_bitrate, desc.max_bitrate)),
			}
		} else {
			return Err(Error::UnsupportedCodec("unknown"));
		};

		Ok(track)
	}

	// Read the media from a stream until processing the moov atom.
	pub async fn init_from<T: AsyncRead + Unpin>(&mut self, input: &mut T) -> Result<()> {
		let _ftyp = mp4_atom::Ftyp::read_from(input).await?;
		let moov = Moov::read_from(input).await?;
		self.init(moov)
	}

	// Read the media from a stream, processing moof and mdat atoms.
	pub async fn read_from<T: AsyncRead + Unpin>(&mut self, input: &mut T) -> Result<()> {
		loop {
			tokio::select! {
				res = Option::<mp4_atom::Any>::read_from(input) => {
					match res {
						Ok(Some(atom)) => self.process(atom)?,
						Ok(None) => return Ok(()),
						Err(err) => return Err(err.into()),
					}
				}
				_ = self.broadcast.closed() => return Err(Error::Closed),
			}
		}
	}

	fn process(&mut self, atom: mp4_atom::Any) -> Result<()> {
		match atom {
			Any::Ftyp(_) | Any::Styp(_) => {
				// Skip
			}
			Any::Moov(moov) => {
				// Create the broadcast.
				self.init(moov)?;
			}
			Any::Moof(moof) => {
				let track_id = util::frame_track_id(&moof)?;
				let keyframe = util::frame_is_key(&moof);

				if keyframe {
					let moov = self.moov.as_ref().ok_or(Error::MissingBox(Moov::KIND))?;
					let trak = moov
						.trak
						.iter()
						.find(|trak| trak.tkhd.track_id == track_id)
						.ok_or(Error::UnknownTrack)?;

					// If this is a video track, start a new group for the keyframe.
					if trak.mdia.hdlr.handler == b"vide".into() {
						// Start a new group for the keyframe.
						for track in self.tracks.values_mut() {
							track.keyframe();
						}
					}
				}

				if self.moof.is_some() {
					// Two moof boxes in a row.
					return Err(Error::DuplicateBox(Moof::KIND));
				}

				self.moof = Some(moof);
			}
			Any::Mdat(mdat) => {
				// Get the track ID from the previous moof.
				let moof = self.moof.take().ok_or(Error::MissingBox(Moof::KIND))?;
				let track_id = util::frame_track_id(&moof)?;
				let timestamp = util::frame_timestamp(&moof)?;

				let track = self.tracks.get_mut(&track_id).ok_or(Error::UnknownTrack)?;
				track.write(timestamp, mdat.data.into());
			}

			_ => {
				// Skip unknown atoms
			}
		};

		Ok(())
	}

	pub fn catalog(&self) -> &catalog::Broadcast {
		self.broadcast.catalog()
	}
}
