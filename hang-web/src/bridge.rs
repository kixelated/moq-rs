use crate::{Result, RoomCommand, RoomStatus};

use tokio::sync::mpsc;
use wasm_bindgen::{prelude::Closure, JsCast};
use web_message::Message;

pub type Command = RoomCommand;
pub type Status = RoomStatus;

pub struct Bridge {
	commands: mpsc::UnboundedReceiver<Command>,
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
					Err(err) => tracing::error!(?err, "Failed to parse command"),
				},
			) as Box<dyn FnMut(_)>);

		global.set_onmessage(Some(closure.as_ref().unchecked_ref()));
		closure.forget();

		Self::send(Status::Init).unwrap();

		Self { commands: rx }
	}

	pub async fn recv(&mut self) -> Option<Command> {
		self.commands.recv().await
	}

	pub fn send(status: Status) -> Result<()> {
		tracing::info!(?status, "sending status");
		let global = js_sys::global().unchecked_into::<web_sys::DedicatedWorkerGlobalScope>();
		let mut transfer = js_sys::Array::new();
		let message = status.into_message(&mut transfer);
		tracing::info!(?message, "sending status");
		global.post_message_with_transfer(&message, &transfer)?;
		Ok(())
	}
}

impl Default for Bridge {
	fn default() -> Self {
		Self::new()
	}
}
