use std::collections::HashMap;

use futures::{stream::FuturesUnordered, StreamExt};
use tokio::sync::mpsc;

use crate::{
	message,
	util::{Close, Lock, OrClose},
	Broadcast, Error, GroupProducer, Produce, Track, TrackConsumer, TrackProducer,
};

use super::{Reader, Stream};

pub fn subscribe(
	id: u64,
	track: Track,
	lookup: Lock<HashMap<u64, SubscribeConsumer>>,
) -> (SubscribeProducer, SubscribeConsumer) {
	let track = track.produce();
	let (tx, rx) = mpsc::channel(1);

	let consumer = SubscribeConsumer {
		groups: tx,
		track: track.1,
	};
	lookup.lock().insert(id, consumer.clone());

	let producer = SubscribeProducer {
		id,
		track: track.0,
		groups: rx,

		parent: lookup,
	};

	(producer, consumer)
}

#[derive(Clone)]
pub struct SubscribeConsumer {
	pub track: TrackConsumer,
	groups: mpsc::Sender<(message::Group, Reader)>,
}

impl SubscribeConsumer {
	pub async fn serve(&self, group: message::Group, stream: Reader) {
		if let Err(err) = self.groups.send((group, stream)).await {
			let (_group, mut stream) = err.0;
			stream.close(Error::Cancel);
		}
	}
}

pub struct SubscribeProducer {
	pub id: u64,
	pub track: TrackProducer,

	groups: mpsc::Receiver<(message::Group, Reader)>,
	parent: Lock<HashMap<u64, SubscribeConsumer>>,
}

impl SubscribeProducer {
	pub async fn start(&mut self, stream: &mut Stream, broadcast: &Broadcast) -> Result<(), Error> {
		let request = message::Subscribe {
			id: self.id,
			broadcast: broadcast.path.clone(),

			track: self.track.name.clone(),
			priority: self.track.priority,

			group_order: self.track.group_order,
			group_expires: self.track.group_expires,

			// TODO
			group_min: None,
			group_max: None,
		};

		stream.writer.encode(&request).await?;

		// TODO use the response to update the track
		let _response: message::Info = stream.reader.decode().await?;

		tracing::info!("ok");

		Ok(())
	}

	pub async fn run(mut self, stream: &mut Stream) -> Result<(), Error> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = stream.reader.decode_maybe::<message::GroupDrop>() => {
					match res? {
						Some(_drop) => {
							// TODO expose updates to application
							// TODO use to detect gaps
						},
						None => return Ok(()),
					}
				},
				Some((group, mut stream)) = self.groups.recv() => {
					let group = self.track.create_group(group.sequence);
					tasks.push(async move {
						Self::serve(group, &mut stream).await.or_close(&mut stream).ok();
					});
				},
				Some(_) = tasks.next() => {},
				// We should close the subscribe because there's no more consumers
				_ = self.track.unused() => return Ok(()),
			};
		}
	}

	#[tracing::instrument("group", skip_all, err, fields(sequence = group.sequence))]
	async fn serve(mut group: GroupProducer, stream: &mut Reader) -> Result<(), Error> {
		while let Some(frame) = stream.decode_maybe::<message::Frame>().await? {
			let mut frame = group.create_frame(frame.size);
			let mut remain = frame.size;

			while remain > 0 {
				let chunk = stream.read(remain).await?.ok_or(Error::WrongSize)?;

				remain = remain.checked_sub(chunk.len()).ok_or(Error::WrongSize)?;
				tracing::trace!(chunk = chunk.len(), remain, "chunk");

				frame.write(chunk);
			}
		}

		Ok(())
	}
}

impl Drop for SubscribeProducer {
	fn drop(&mut self) {
		self.parent.lock().remove(&self.id);
	}
}
