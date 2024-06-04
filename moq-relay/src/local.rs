use std::collections::hash_map;
use std::collections::HashMap;

use std::sync::{Arc, Mutex};

use moq_transfork::serve::{BroadcastReader, ServeError};

#[derive(Clone)]
pub struct Locals {
	lookup: Arc<Mutex<HashMap<String, BroadcastReader>>>,
}

impl Default for Locals {
	fn default() -> Self {
		Self::new()
	}
}

impl Locals {
	pub fn new() -> Self {
		Self {
			lookup: Default::default(),
		}
	}

	pub async fn register(&mut self, broadcast: BroadcastReader) -> anyhow::Result<Registration> {
		let name = broadcast.name.clone();
		match self.lookup.lock().unwrap().entry(name.clone()) {
			hash_map::Entry::Vacant(entry) => entry.insert(broadcast),
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		let registration = Registration {
			locals: self.clone(),
			broadcast: name,
		};

		Ok(registration)
	}

	pub fn route(&self, broadcast: &str) -> Option<BroadcastReader> {
		self.lookup.lock().unwrap().get(broadcast).cloned()
	}
}

pub struct Registration {
	locals: Locals,
	broadcast: String,
}

impl Drop for Registration {
	fn drop(&mut self) {
		self.locals.lookup.lock().unwrap().remove(&self.broadcast);
	}
}
