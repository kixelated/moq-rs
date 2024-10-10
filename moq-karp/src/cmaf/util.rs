use mp4_atom::{Atom, Moof, Tfdt, Traf};

use super::Error;

pub fn frame_is_key(moof: &Moof) -> bool {
	for traf in &moof.traf {
		// TODO trak default flags if this is None
		let default_flags = traf.tfhd.default_sample_flags.unwrap_or_default();
		let trun = match &traf.trun {
			Some(t) => t,
			None => return false,
		};

		for entry in trun.entries.iter() {
			let flags = entry.flags.unwrap_or(default_flags);

			// https://chromium.googlesource.com/chromium/src/media/+/master/formats/mp4/track_run_iterator.cc#177
			let keyframe = (flags >> 24) & 0x3 == 0x2; // kSampleDependsOnNoOther
			let non_sync = (flags >> 16) & 0x1 == 0x1; // kSampleIsNonSyncSample

			if keyframe && !non_sync {
				return true;
			}
		}
	}

	false
}

pub fn frame_timestamp(moof: &Moof) -> Result<u64, Error> {
	let traf = match moof.traf[..] {
		[ref traf] => traf,
		[] => return Err(Error::MissingBox(Traf::KIND)),
		_ => return Err(Error::DuplicateBox(Traf::KIND)),
	};

	let tfdt = traf.tfdt.as_ref().ok_or(Error::MissingBox(Tfdt::KIND))?;
	Ok(tfdt.base_media_decode_time)
}

pub fn frame_track_id(moof: &Moof) -> Result<u32, Error> {
	let traf = match moof.traf[..] {
		[ref traf] => traf,
		[] => return Err(Error::MissingBox(Traf::KIND)),
		_ => return Err(Error::DuplicateBox(Traf::KIND)),
	};

	let track_id = traf.tfhd.track_id;
	Ok(track_id)
}
