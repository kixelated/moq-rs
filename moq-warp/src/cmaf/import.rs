use bytes::BytesMut;
use mp4_atom::{Any, AsyncReadFrom, Atom, Esds, Ftyp, Moof, Moov, Trak};
use std::collections::HashMap;
use tokio::io::AsyncRead;

use super::{util, Error, Result};
use crate::{catalog, media};

/// Converts fMP4 -> Warp
pub struct Import<R: AsyncRead + Unpin> {
	// The input file
	input: R,

	// The broadcast being produced
	broadcast: media::BroadcastProducer,

	// A lookup to tracks in the broadcast
	tracks: HashMap<u32, media::TrackProducer>,

	// The moov atom at the start of the file.
	moov: Moov,

	// The latest moof header
	moof: Option<Moof>,
}

impl<R: AsyncRead + Unpin> Import<R> {
	pub async fn init(mut input: R, output: moq_transfork::BroadcastProducer) -> Result<Self> {
		let _ = Ftyp::read_from(&mut input).await?;
		let moov = Moov::read_from(&mut input).await?;

		let mut broadcast = media::BroadcastProducer::new(output);
		let mut tracks = HashMap::default();

		// Produce the catalog
		for trak in &moov.trak {
			let track_id = trak.tkhd.track_id;
			let handler = &trak.mdia.hdlr.handler_type;

			let track = match handler.as_ref() {
				b"vide" => {
					let track = Self::init_video(&trak)?;
					broadcast.create_video(track)?
				}
				b"soun" => {
					let track = Self::init_audio(&trak)?;
					broadcast.create_audio(track)?
				}
				b"sbtl" => return Err(Error::UnsupportedTrack("subtitle")),
				_ => return Err(Error::UnsupportedTrack("unknown")),
			};

			tracks.insert(track_id, track);
		}

		broadcast.publish()?;

		Ok(Import {
			input,
			broadcast,
			tracks,
			moov,
			moof: None,
		})
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
			let hvcc = &hev1.hvcc;

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

	// Read the media from a stream, processing moof and mdat atoms.
	pub async fn run(mut self) -> Result<()> {
		while let Some(atom) = Option::<mp4_atom::Any>::read_from(&mut self.input).await? {
			self.process(atom)?;
		}

		Ok(())
	}

	fn process(&mut self, atom: mp4_atom::Any) -> Result<()> {
		match atom {
			Any::Ftyp(_) => {
				return Err(Error::DuplicateBox(Ftyp::KIND));
			}
			Any::Moov(_) => {
				return Err(Error::DuplicateBox(Moov::KIND));
			}
			Any::Moof(moof) => {
				let track_id = util::frame_track_id(&moof)?;
				let keyframe = util::frame_is_key(&moof);

				if keyframe {
					let trak = self
						.moov
						.trak
						.iter()
						.find(|trak| trak.tkhd.track_id == track_id)
						.ok_or(Error::UnknownTrack)?;

					// If this is a video track, start a new group for the keyframe.
					if trak.mdia.hdlr.handler_type == b"vide".into() {
						// Start a new group for the keyframe.
						for track in self.tracks.values_mut() {
							track.keyframe();
						}
					}
				}

				if self.moof.is_some() {
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
				track.write(timestamp, mdat.data);
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
