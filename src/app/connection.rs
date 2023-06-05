use std::collections::HashMap;

use std::time;

use quiche;
use quiche::h3::webtransport;

use super::message;
use crate::{media, transport};

pub struct Connection {
	// The underlying QUIC connection
	quic: quiche::Connection,

	// The WebTransport session on top of the QUIC connection
	session: webtransport::ServerSession,

	// A helper for automatically buffering stream data.
	streams: transport::Streams,

	// Each active track, with the key as the track_id.
	tracks: HashMap<u32, media::track::Subscriber>,

	// Each active segment with the key as the stream_id.
	segments: HashMap<u64, media::segment::Subscriber>,
}

impl Connection {
	pub fn new(
		quic: quiche::Connection,
		session: webtransport::ServerSession,
		mut broadcast: media::broadcast::Subscriber,
	) -> Self {
		Self {
			quic,
			session,
			tracks: broadcast.tracks(),
			segments: HashMap::new(),
			streams: transport::Streams::new(),
		}
	}
}

impl transport::app::Connection for Connection {
	fn conn(&mut self) -> &mut quiche::Connection {
		&mut self.quic
	}

	// Process any updates to the connection.
	fn poll(&mut self) -> anyhow::Result<Option<time::Duration>> {
		self.poll_transport()?;

		let timeout = self.poll_media()?;

		// Send any pending stream data.
		// TODO: return any errors
		self.streams.poll(&mut self.quic);

		Ok(timeout)
	}
}

impl Connection {
	fn poll_transport(&mut self) -> anyhow::Result<()> {
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

		Ok(())
	}

	fn poll_media(&mut self) -> anyhow::Result<Option<time::Duration>> {
		for (track_id, track_sub) in &mut self.tracks {
			while let Some(segment) = track_sub.segment() {
				// Create a new unidirectional stream.
				let stream_id = self.session.open_stream(&mut self.quic, false)?;

				// TODO send order
				// Set the stream priority to be equal to the timestamp.
				// We subtract from u64::MAX so newer media is sent important.
				// TODO prioritize audio
				// let order = u64::MAX - fragment.timestamp;
				// self.streams.send_order(conn, stream_id, order);

				// Encode a JSON header indicating this is a new track.
				let mut message: message::Message = message::Message::new();
				message.segment = Some(message::Segment { track_id: *track_id });

				// Write the header.
				let data = message.serialize()?;
				self.streams.send(&mut self.quic, stream_id, &data, false)?;

				let segment = media::segment::Subscriber::new(segment);
				self.segments.insert(stream_id, segment);
			}
		}

		self.tracks.retain(|_, track_sub| !track_sub.done());

		for (stream_id, segment_sub) in &mut self.segments {
			while let Some(fragment) = segment_sub.fragment() {
				// Write the fragment.
				self.streams
					.send(&mut self.quic, *stream_id, fragment.as_slice(), false)?;
			}

			// TODO combine with the retain call below
			if segment_sub.done() {
				// Close the stream
				self.streams.send(&mut self.quic, *stream_id, &[], true)?;
			}
		}

		self.segments.retain(|_stream_id, segment_sub| !segment_sub.done());

		// TODO implement futures...
		Ok(time::Duration::from_millis(10).into())
	}
}
