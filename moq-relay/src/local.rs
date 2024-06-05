use std::collections::hash_map;
use std::collections::HashMap;

use std::sync::{Arc, Mutex};

use moq_transfork::serve::ServeError;
use moq_transfork::serve::UnknownReader;

#[derive(Clone)]
pub struct Locals {
	lookup: Arc<Mutex<HashMap<String, UnknownReader>>>,
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

	pub fn register(&mut self, broadcast: &str, tracks: UnknownReader) -> anyhow::Result<Registration> {
		let name = broadcast.to_string();

		match self.lookup.lock().unwrap().entry(name.clone()) {
			hash_map::Entry::Vacant(entry) => entry.insert(tracks),
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		let registration = Registration {
			locals: self.clone(),
			broadcast: name,
		};

		Ok(registration)
	}

	pub fn route(&self, broadcast: &str) -> Option<UnknownReader> {
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
