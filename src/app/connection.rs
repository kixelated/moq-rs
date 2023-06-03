use std::collections::hash_map as hmap;

use std::time;

use quiche;
use quiche::h3::webtransport;


use crate::{media, transport};

pub struct Connection {
	// The underlying QUIC connection
	quic: quiche::Connection,

	// The WebTransport session on top of the QUIC connection
	session: webtransport::ServerSession,

	// The media subscription
	media: media::Subscription,

	// A helper for automatically buffering stream data.
	streams: transport::Streams,

	// Map from track_id to the the Track state.
	tracks: hmap::HashMap<u32, Track>,
}

pub struct Track {
	// Current stream_id
	stream_id: Option<u64>,

	// The timescale used for this track.
	timescale: u64,

	// The timestamp of the last keyframe.
	keyframe: u64,
}

impl Connection {
	pub fn new(quic: quiche::Connection, session: webtransport::ServerSession, media: media::Subscription) -> Self {
		Self {
			quic,
			session,
			media,
			streams: transport::Streams::new(),
			tracks: hmap::HashMap::new(),
		}
	}
}

impl transport::app::Connection for Connection {
	fn conn(&mut self) -> &mut quiche::Connection {
		&mut self.quic
	}

	// Process any updates to the connection.
	fn poll(&mut self) -> anyhow::Result<Option<time::Duration>> {
		loop {
			let event = match self.session.poll(&mut self.quic) {
				Err(webtransport::Error::Done) => break,
				Err(e) => return Err(e.into()),
				Ok(e) => e,
			};

			log::debug!("webtransport event {:?}", event);

			match event {
				webtransport::ServerEvent::ConnectRequest(_req) => {
					// you can handle request with
					// req.authority()
					// req.path()
					// and you can validate this request with req.origin()
					self.session.accept_connect_request(&mut self.quic, None)?;
					/*

					// TODO
					let media = media::Source::new("media/fragmented.mp4").expect("failed to open fragmented.mp4");
					let init = &media.init;

					// Create a JSON header.
					let mut message = message::Message::new();
					message.init = Some(message::Init {});
					let data = message.serialize()?;

					// Create a new stream and write the header.
					let stream_id = session.open_stream(conn, false)?;
					self.streams.send(conn, stream_id, data.as_slice(), false)?;
					self.streams.send(conn, stream_id, init.as_slice(), true)?;
					*/

					// TODO

					//self.media = Some(media);
				}
				webtransport::ServerEvent::StreamData(stream_id) => {
					let mut buf = vec![0; 10000];
					while let Ok(len) = self.session.recv_stream_data(&mut self.quic, stream_id, &mut buf) {
						let _stream_data = &buf[0..len];
					}
				}

				_ => {}
			}
		}

		// Send any pending stream data.
		// NOTE: This doesn't return an error because it's async, and would be confusing.
		self.streams.poll(&mut self.quic);

		Ok(None)
	}
}

impl Connection {
	/*
	fn poll_source(&mut self, session: &mut webtransport::ServerSession) -> anyhow::Result<()> {
		// Get the next media fragment.
		let fragment = match self.media.fragment()? {
			Some(f) => f,
			None => return Ok(()),
		};

		// Get the track state or insert a new entry.
		let track = self.tracks.entry(fragment.track_id).or_insert_with(|| Track {
			stream_id: None,
			timescale: fragment.timescale,
			keyframe: 0,
		});

		if let Some(stream_id) = track.stream_id {
			// Existing stream, check if we should close it.
			if fragment.keyframe && fragment.timestamp >= track.keyframe + track.timescale {
				// Close the existing stream
				self.streams.send(conn, stream_id, &[], true)?;

				// Unset the stream id so we create a new one.
				track.stream_id = None;
				track.keyframe = fragment.timestamp;
			}
		}

		let stream_id = match track.stream_id {
			Some(stream_id) => stream_id,
			None => {
				// Create a new unidirectional stream.
				let stream_id = session.open_stream(conn, false)?;

				// Set the stream priority to be equal to the timestamp.
				// We subtract from u64::MAX so newer media is sent important.
				// TODO prioritize audio
				let order = u64::MAX - fragment.timestamp;
				self.streams.send_order(conn, stream_id, order);

				// Encode a JSON header indicating this is a new track.
				let mut message: message::Message = message::Message::new();
				message.segment = Some(message::Segment {
					track_id: fragment.track_id,
				});

				// Write the header.
				let data = message.serialize()?;
				self.streams.send(conn, stream_id, &data, false)?;

				stream_id
			}
		};

		// Write the current fragment.
		let data = fragment.data.as_slice();
		self.streams.send(conn, stream_id, data, false)?;

		// Save the stream_id for the next fragment.
		track.stream_id = Some(stream_id);

		Ok(())
	}
	*/
}
