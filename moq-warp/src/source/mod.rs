mod file;
pub use file::*;

use crate::model::track;

pub trait Source {
	fn subscribe(&self, name: &str) -> Option<track::Subscriber>;
}
