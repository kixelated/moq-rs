use bytes::{Bytes, BytesMut};
use mp4_atom::{Any, AsyncReadFrom, Atom, DecodeMaybe, Esds, Mdat, Moof, Moov, Tfdt, Trak, Trun};
use std::{collections::HashMap, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt};

use super::{Error, Result};
use crate::{Audio, BroadcastProducer, Dimensions, Frame, Timestamp, Track, TrackProducer, Video, AAC, H264, VP9};

/// Converts fMP4 -> Karp
pub struct Import {
	// Any partial data in the input buffer
	buffer: BytesMut,

	// The broadcast being produced
	broadcast: BroadcastProducer,

	// A lookup to tracks in the broadcast
	tracks: HashMap<u32, TrackProducer>,

	// The timestamp of the last keyframe for each track
	last_keyframe: HashMap<u32, Timestamp>,

	// The moov atom at the start of the file.
	moov: Option<Moov>,

	// The latest moof header
	moof: Option<Moof>,
	moof_size: usize,
}

impl Import {
	pub fn new(broadcast: BroadcastProducer) -> Self {
		Self {
			buffer: BytesMut::new(),
			broadcast,
			tracks: HashMap::default(),
			last_keyframe: HashMap::default(),
			moov: None,
			moof: None,
			moof_size: 0,
		}
	}

	pub fn parse(&mut self, data: &[u8]) -> Result<()> {
		if !self.buffer.is_empty() {
			let mut buffer = std::mem::replace(&mut self.buffer, BytesMut::new());
			buffer.extend_from_slice(data);
			let n = self.parse_inner(&buffer)?;
			self.buffer = buffer.split_off(n);
		} else {
			let n = self.parse_inner(data)?;
			self.buffer = BytesMut::from(&data[n..]);
		}

		Ok(())
	}

	fn parse_inner<T: AsRef<[u8]>>(&mut self, data: T) -> Result<usize> {
		let mut remain = data.as_ref();

		loop {
			let mut peek = remain;

			match mp4_atom::Any::decode_maybe(&mut peek)? {
				Some(atom) => {
					self.process(atom, remain.len() - peek.len())?;
					remain = peek;
				}
				None => break,
			}
		}

		// Return the number of bytes consumed
		Ok(data.as_ref().len() - remain.len())
	}

	fn init(&mut self, moov: Moov) -> Result<()> {
		// Produce the catalog
		for trak in &moov.trak {
			let track_id = trak.tkhd.track_id;
			let handler = &trak.mdia.hdlr.handler;

			let track = match handler.as_ref() {
				b"vide" => {
					let track = Self::init_video(trak)?;
					self.broadcast.publish_video(track)?
				}
				b"soun" => {
					let track = Self::init_audio(trak)?;
					self.broadcast.publish_audio(track)?
				}
				b"sbtl" => return Err(Error::UnsupportedTrack("subtitle")),
				_ => return Err(Error::UnsupportedTrack("unknown")),
			};

			self.tracks.insert(track_id, track);
		}

		self.moov = Some(moov);

		Ok(())
	}

	fn init_video(trak: &Trak) -> Result<Video> {
		let name = format!("video{}", trak.tkhd.track_id);
		let stsd = &trak.mdia.minf.stbl.stsd;

		let track = if let Some(avc1) = &stsd.avc1 {
			let avcc = &avc1.avcc;

			let mut description = BytesMut::new();
			avcc.encode_body(&mut description)?;

			Video {
				track: Track { name, priority: 2 },
				resolution: Dimensions {
					width: avc1.width as _,
					height: avc1.height as _,
				},
				codec: H264 {
					profile: avcc.avc_profile_indication,
					constraints: avcc.profile_compatibility,
					level: avcc.avc_level_indication,
				}
				.into(),
				description: Some(description.freeze()),
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

			Video {
				track: Track { name, priority: 2 },
				codec: VP9 {
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
				description: Default::default(),
				resolution: Dimensions {
					width: vp09.width as _,
					height: vp09.height as _,
				},
				bitrate: None,
			}
		} else {
			// TODO add av01 support: https://github.com/gpac/mp4box.js/blob/325741b592d910297bf609bc7c400fc76101077b/src/box-codecs.js#L251
			return Err(Error::UnsupportedCodec("unknown"));
		};

		Ok(track)
	}

	fn init_audio(trak: &Trak) -> Result<Audio> {
		let name = format!("audio{}", trak.tkhd.track_id);
		let stsd = &trak.mdia.minf.stbl.stsd;

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

			Audio {
				track: Track { name, priority: 1 },
				codec: AAC {
					profile: desc.dec_specific.profile,
				}
				.into(),
				sample_rate: mp4a.samplerate.integer() as _,
				channel_count: mp4a.channelcount as _,
				bitrate: Some(std::cmp::max(desc.avg_bitrate, desc.max_bitrate) as _),
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
	pub async fn read_from<T: AsyncReadExt + Unpin>(&mut self, input: &mut T) -> Result<()> {
		let mut buffer = BytesMut::new();

		while input.read_buf(&mut buffer).await? > 0 {
			let n = self.parse_inner(&buffer)?;
			let _ = buffer.split_to(n);
		}

		if !buffer.is_empty() {
			return Err(Error::TrailingData);
		}

		Ok(())
	}

	fn process(&mut self, atom: mp4_atom::Any, size: usize) -> Result<()> {
		match atom {
			Any::Ftyp(_) | Any::Styp(_) => {
				// Skip
			}
			Any::Moov(moov) => {
				// Create the broadcast.
				self.init(moov)?;
			}
			Any::Moof(moof) => {
				if self.moof.is_some() {
					// Two moof boxes in a row.
					return Err(Error::DuplicateBox(Moof::KIND));
				}

				self.moof = Some(moof);
				self.moof_size = size;
			}
			Any::Mdat(mdat) => {
				// Extract the samples from the mdat atom.
				let header_size = size - mdat.data.len();
				self.extract(mdat, header_size)?;
			}
			_ => {
				// Skip unknown atoms
				tracing::warn!(?atom, "skipping")
			}
		};

		Ok(())
	}

	// Extract all frames out of an mdat atom.
	fn extract(&mut self, mdat: Mdat, header_size: usize) -> Result<()> {
		let mdat = Bytes::from(mdat.data);
		let moov = self.moov.as_ref().ok_or(Error::MissingBox(Moov::KIND))?;
		let moof = self.moof.take().ok_or(Error::MissingBox(Moof::KIND))?;

		// Keep track of the minimum and maximum timestamp so we can scold the user.
		// Ideally these should both be the same value.
		let mut min_timestamp = None;
		let mut max_timestamp = None;

		// Loop over all of the traf boxes in the moof.
		for traf in &moof.traf {
			let track_id = traf.tfhd.track_id;
			let track = self.tracks.get_mut(&track_id).ok_or(Error::UnknownTrack)?;

			// Find the track information in the moov
			let trak = moov
				.trak
				.iter()
				.find(|trak| trak.tkhd.track_id == track_id)
				.ok_or(Error::UnknownTrack)?;
			let trex = moov
				.mvex
				.as_ref()
				.and_then(|mvex| mvex.trex.iter().find(|trex| trex.track_id == track_id));

			// The moov contains some defaults
			let default_sample_duration = trex.map(|trex| trex.default_sample_duration).unwrap_or_default();
			let default_sample_size = trex.map(|trex| trex.default_sample_size).unwrap_or_default();
			let default_sample_flags = trex.map(|trex| trex.default_sample_flags).unwrap_or_default();

			let tfhd = &traf.tfhd;
			let trun = traf.trun.as_ref().ok_or(Error::MissingBox(Trun::KIND))?;

			let tfdt = traf.tfdt.as_ref().ok_or(Error::MissingBox(Tfdt::KIND))?;
			let mut dts = tfdt.base_media_decode_time;
			let timescale = trak.mdia.mdhd.timescale as u64;

			let mut offset = tfhd.base_data_offset.unwrap_or_default() as usize;

			if let Some(data_offset) = trun.data_offset {
				// This is relative to the start of the MOOF, not the MDAT.
				// Note: The trun data offset can be negative, but... that's not supported here.
				let data_offset: usize = data_offset.try_into().map_err(|_| Error::InvalidOffset)?;
				if data_offset < self.moof_size {
					return Err(Error::InvalidOffset);
				}

				offset += data_offset - self.moof_size - header_size;
			}

			for entry in &trun.entries {
				// Use the moof defaults if the sample doesn't have its own values.
				let flags = entry
					.flags
					.unwrap_or(tfhd.default_sample_flags.unwrap_or(default_sample_flags));
				let duration = entry
					.duration
					.unwrap_or(tfhd.default_sample_duration.unwrap_or(default_sample_duration));
				let size = entry
					.size
					.unwrap_or(tfhd.default_sample_size.unwrap_or(default_sample_size)) as usize;

				let pts = (dts as i64 + entry.cts.unwrap_or_default() as i64) as u64;
				let timestamp = Timestamp::from_micros(1_000_000 * pts / timescale);

				if offset + size > mdat.len() {
					return Err(Error::InvalidOffset);
				}

				let keyframe = if trak.mdia.hdlr.handler == b"vide".into() {
					// https://chromium.googlesource.com/chromium/src/media/+/master/formats/mp4/track_run_iterator.cc#177
					let keyframe = (flags >> 24) & 0x3 == 0x2; // kSampleDependsOnNoOther
					let non_sync = (flags >> 16) & 0x1 == 0x1; // kSampleIsNonSyncSample

					if keyframe && !non_sync {
						for audio in moov.trak.iter().filter(|t| t.mdia.hdlr.handler == b"soun".into()) {
							// Force an audio keyframe on video keyframes
							self.last_keyframe.remove(&audio.tkhd.track_id);
						}

						true
					} else {
						false
					}
				} else {
					match self.last_keyframe.get(&track_id) {
						// Force an audio keyframe at least every 10 seconds, but ideally at video keyframes
						Some(prev) => timestamp - *prev > Duration::from_secs(10),
						None => true,
					}
				};

				if keyframe {
					self.last_keyframe.insert(track_id, timestamp);
				}

				let payload = mdat.slice(offset..(offset + size));

				let frame = Frame {
					timestamp,
					keyframe,
					payload,
				};
				track.write(frame);

				dts += duration as u64;
				offset += size;

				if timestamp >= max_timestamp.unwrap_or_default() {
					max_timestamp = Some(timestamp);
				}
				if timestamp <= min_timestamp.unwrap_or_default() {
					min_timestamp = Some(timestamp);
				}
			}
		}

		if let (Some(min), Some(max)) = (min_timestamp, max_timestamp) {
			let diff = max - min;

			if diff > Duration::from_millis(1) {
				tracing::warn!("fMP4 introduced {:?} of latency", diff);
			}
		}

		Ok(())
	}
}
