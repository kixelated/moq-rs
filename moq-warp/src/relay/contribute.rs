use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::task::JoinSet; // lock across await boundaries

use moq_transport::coding::VarInt;
use moq_transport::{control, object};

use anyhow::Context;

use crate::{broadcast, broadcasts};

pub struct Contribute {
	// Used to receive objects.
	// TODO split into send/receive halves.
	transport: Arc<object::Transport>,

	// Used to send control messages.
	control: Arc<Mutex<control::SendStream>>,

	// Globally announced namespaces, which we can add ourselves to.
	broadcasts: broadcasts::Shared,

	/*
	// The session announced these namespaces, so we can subscribe to them if needed.
	broadcasts: HashMap<String, broadcast::Publisher>,

	// Incoming objects with this ID are published to the given track.
	tracks: HashMap<VarInt, track::Publisher>,
	*/
	// A list of tasks that are currently running.
	tasks: JoinSet<anyhow::Result<()>>,
}

impl Contribute {
	pub fn new(
		transport: Arc<object::Transport>,
		control: Arc<Mutex<control::SendStream>>,
		broadcasts: broadcasts::Shared,
	) -> Self {
		Self {
			transport,
			control,
			broadcasts,
			//tracks: HashMap::new(),
			tasks: JoinSet::new(),
		}
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				res = self.tasks.join_next(), if !self.tasks.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::error!("failed to serve subscription: {:?}", err);
					}
				},
				object = self.transport.recv() => {
					let (header, stream )= object.context("failed to receive object")?;
					self.receive_object(header, stream).await?;
				},
			}
		}
	}

	pub async fn receive_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		match msg {
			control::Message::Announce(msg) => self.receive_announce(msg).await,
			control::Message::SubscribeOk(msg) => self.receive_subscribe_ok(msg),
			control::Message::SubscribeError(msg) => self.receive_subscribe_error(msg),
			// TODO make this type safe
			_ => anyhow::bail!("invalid message for contribution: {:?}", msg),
		}
	}

	async fn receive_object(&mut self, header: object::Header, mut stream: object::RecvStream) -> anyhow::Result<()> {
		todo!("receive object")
		/*
		let producer = self
			.subscribed
			.get_mut(&header.track_id)
			.expect("no subscription with that ID");

		let mut fragments = Publisher::new();

		producer.segments.push(Segment {
			sequence: header.object_sequence,
			send_order: header.send_order,
			expires: Some(time::Instant::now() + time::Duration::from_secs(10)),
			fragments: fragments.subscribe(),
		});

		// TODO implement a timeout

		self.tasks.spawn(async move {
			let mut buf = [0u8; 32 * 1024];
			loop {
				let size = stream.read(&mut buf).await.context("failed to read from stream")?;
				if size == 0 {
					break;
				}

				let chunk = buf[..size].to_vec();
				fragments.push(chunk.into())
			}

			Ok(())
		});

		Ok(())
		*/
	}

	async fn receive_announce(&mut self, msg: control::Announce) -> anyhow::Result<()> {
		match self.receive_announce_inner(&msg).await {
			Ok(()) => {
				self.send_message(control::AnnounceOk {
					track_namespace: msg.track_namespace,
				})
				.await
			}
			Err(e) => {
				self.send_message(control::AnnounceError {
					track_namespace: msg.track_namespace,
					code: VarInt::from_u32(1),
					reason: e.to_string(),
				})
				.await
			}
		}
	}

	async fn receive_announce_inner(&mut self, msg: &control::Announce) -> anyhow::Result<()> {
		let (publisher, subscriber) = broadcast::new(msg.track_namespace.clone());

		// Check if this broadcast already exists globally.
		let mut broadcasts = self.broadcasts.lock().await;
		if broadcasts.contains_key(&msg.track_namespace) {
			anyhow::bail!("duplicate broadcast: {}", msg.track_namespace);
		}

		// Insert the subscriber into the global list of broadcasts.
		broadcasts.insert(subscriber.namespace.clone(), subscriber);

		// TODO kill this task on unannounce
		self.tasks.spawn(async move { Self::serve_announce(publisher).await });

		Ok(())
	}

	fn receive_subscribe_ok(&mut self, _msg: control::SubscribeOk) -> anyhow::Result<()> {
		Ok(())
	}

	fn receive_subscribe_error(&mut self, msg: control::SubscribeError) -> anyhow::Result<()> {
		// TODO make sure we sent this subscribe
		anyhow::bail!("received SUBSCRIBE_ERROR({:?}): {}", msg.code, msg.reason)
	}

	async fn serve_announce(broadcast: broadcast::Publisher) -> anyhow::Result<()> {
		todo!("serve announce")
	}

	async fn send_message<T: Into<control::Message>>(&mut self, msg: T) -> anyhow::Result<()> {
		// We use a tokio mutex so we can hold the lock across await (when control stream is full).
		let mut control = self.control.lock().await;

		let msg = msg.into();
		log::info!("sending message: {:?}", msg);
		control.send(msg).await
	}
}
