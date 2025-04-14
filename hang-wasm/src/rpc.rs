use crate::{Connect, Publish, PublishCommand, Result, Watch, WatchCommand};

use hang::Room;
use tokio::sync::mpsc;
use ts_rs::TS;
use url::Url;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_message::Message;

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/rpc.ts")]
pub enum Event {
	Init,
}

#[derive(Debug, Message, TS)]
#[ts(export, export_to = "../src/rpc.ts")]
pub enum Command {
	Connect { url: Option<Url> },
	Publish(PublishCommand),
	Watch(WatchCommand),
}

pub struct Backend {
	commands: mpsc::UnboundedReceiver<Command>,
	connecting: Option<Connect>,
	publish: Publish,
	watch: Watch,
}

impl Default for Backend {
	fn default() -> Self {
		Self::new()
	}
}

impl Backend {
	pub fn new() -> Self {
		// Create a channel to receive commands from the worker.
		let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

		// Get the worker Worker scope
		let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();

		let closure =
			Closure::wrap(Box::new(
				move |event: web_sys::MessageEvent| match Command::from_message(event.data()) {
					Ok(command) => tx.send(command).unwrap(),
					Err(err) => tracing::error!(?err, "Failed to parse command"),
				},
			) as Box<dyn FnMut(_)>);

		global.set_onmessage(Some(closure.as_ref().unchecked_ref()));
		closure.forget();

		Self {
			commands: rx,
			connecting: None,
			publish: Publish::new(),
			watch: Watch::default(),
		}
	}

	pub async fn run(&mut self) -> Result<()> {
		loop {
			tokio::select! {
				command = self.commands.recv() => {
					self.recv_command(command.expect("closed")).await?;
				},
				Some(session) = async { Some(self.connecting.as_mut()?.established().await) } => {
					tracing::info!("connected to server");
					let session = session?;
					let path = self.connecting.take().unwrap().path;
					let room = Room::new(session, path);

					self.watch.set_room(Some(room.clone()));
					self.publish.set_room(Some(room))?;
				},
				Err(err) = self.watch.run() => return Err(err),
				Err(err) = self.publish.run() => return Err(err),
				else => return Ok(()),
			}
		}
	}

	async fn recv_command(&mut self, command: Command) -> Result<()> {
		tracing::info!(?command, "Received command");

		match command {
			Command::Connect { url } => {
				if let Some(url) = url {
					self.connecting = Some(Connect::new(url));
				} else {
					self.connecting = None;
				}

				self.watch.set_room(None);
				self.publish.set_room(None)?;
			}
			Command::Watch(command) => {
				self.watch.recv_command(command);
			}
			Command::Publish(command) => {
				self.publish.recv_command(command).await?;
			}
		}

		Ok(())
	}

	pub fn send_event(&self, event: Event) -> Result<()> {
		tracing::info!(?event, "Sending event");
		let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();
		let mut transfer = js_sys::Array::new();
		let message = event.into_message(&mut transfer);
		global.post_message_with_transfer(&message, &transfer)?;
		Ok(())
	}
}
