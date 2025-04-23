use crate::{Command, Event, Publish, Result, Watch};

use tokio::sync::mpsc;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_message::Message;

pub struct Bridge {
	commands: mpsc::UnboundedReceiver<Command>,
	watch: Watch,
	publish: Publish,
}

impl Bridge {
	pub fn new() -> Self {
		// Create a channel to receive commands from the worker.
		let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

		// Get the worker Worker scope
		let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();

		let closure =
			Closure::wrap(Box::new(
				move |event: web_sys::MessageEvent| match Command::from_message(event.data()) {
					Ok(command) => tx.send(command).unwrap(),
					Err(err) => tracing::error!(?err, "failed to parse command"),
				},
			) as Box<dyn FnMut(_)>);

		global.set_onmessage(Some(closure.as_ref().unchecked_ref()));
		closure.forget();

		Self::send(Event::Init).unwrap();

		Self {
			commands: rx,
			watch: Watch::default(),
			publish: Publish::default(),
		}
	}

	pub fn send(event: Event) -> Result<()> {
		let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();
		let mut transfer = js_sys::Array::new();
		let msg = event.into_message(&mut transfer);
		tracing::info!(?msg, "sending event");
		global.post_message_with_transfer(&msg, &transfer)?;
		Ok(())
	}

	pub async fn run(mut self) -> Result<()> {
		loop {
			tokio::select! {
				cmd = self.commands.recv() => {
					let cmd = cmd.unwrap();
					tracing::debug!(?cmd, "received command");

					match cmd {
						Command::Publish(command) => {
							if let Err(err) = self.publish.recv(command).await {
								tracing::error!(?err, "failed to process publish command");
							}
						}
						Command::Watch(command) => {
							if let Err(err) = self.watch.recv(command) {
								tracing::error!(?err, "failed to process watch command");
							}
						}
					}
				}
				Err(err) = self.watch.run() => return Err(err),
				Err(err) = self.publish.run() => return Err(err),
			}
		}
	}
}

impl Default for Bridge {
	fn default() -> Self {
		Self::new()
	}
}
