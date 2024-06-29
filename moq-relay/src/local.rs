use std::collections::hash_map;
use std::collections::HashMap;

use std::sync::{Arc, Mutex};

use moq_transfork::BroadcastReader;
use moq_transfork::Closed;

#[derive(Clone)]
pub struct Locals {
	broadcasts: Arc<Mutex<HashMap<String, BroadcastReader>>>,
	//root: Option<moq_transfork::Publisher>,
	//host: Option<String>,
}

impl Default for Locals {
	fn default() -> Self {
		Self::new()
	}
}

impl Locals {
	pub fn new(/*root: Option<Publisher>, host: Option<String>*/) -> Self {
		Self {
			broadcasts: Default::default(),
			//root,
			//host,
		}
	}

	//pub fn run(mut self) -> anyhow::Result<()> {}

	pub fn announce(&mut self, broadcast: BroadcastReader) -> anyhow::Result<LocalRegistration> {
		let name = broadcast.name.clone();

		match self.broadcasts.lock().unwrap().entry(name.clone()) {
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast),
			hash_map::Entry::Occupied(_) => return Err(Closed::Duplicate.into()),
		};

		let registration = LocalRegistration {
			locals: self.clone(),
			broadcast: name,
		};

		Ok(registration)
	}

	pub fn route(&self, broadcast: &str) -> Option<BroadcastReader> {
		tracing::info!("routing broadcast: {:?}", broadcast);
		self.broadcasts.lock().unwrap().get(broadcast).cloned()
	}
}

pub struct LocalRegistration {
	locals: Locals,
	broadcast: String,
}

impl Drop for LocalRegistration {
	fn drop(&mut self) {
		self.locals.broadcasts.lock().unwrap().remove(&self.broadcast);
	}
}
