use std::sync::Arc;

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Path {
	parts: Vec<Arc<String>>,
}

impl Path {
	pub fn new(parts: Vec<String>) -> Path {
		Path {
			parts: parts.into_iter().map(Arc::new).collect(),
		}
	}

	pub fn push<T: ToString>(mut self, part: T) -> Self {
		self.parts.push(Arc::new(part.to_string()));
		self
	}

	pub fn append(mut self, other: &Path) -> Self {
		self.parts.extend_from_slice(&other.parts);
		self
	}

	pub fn has_prefix(&self, prefix: &Path) -> bool {
		if prefix.parts.len() > self.parts.len() {
			return false;
		}

		prefix.parts.iter().zip(self.parts.iter()).all(|(a, b)| a == b)
	}

	pub fn strip_prefix(mut self, prefix: &Path) -> Option<Self> {
		if !self.has_prefix(prefix) {
			return None;
		}

		self.parts.drain(..prefix.parts.len());
		Some(self)
	}

	pub fn has_suffix(&self, suffix: &Path) -> bool {
		if suffix.parts.len() > self.parts.len() {
			return false;
		}

		suffix
			.parts
			.iter()
			.rev()
			.zip(self.parts.iter().rev())
			.all(|(a, b)| a == b)
	}

	pub fn strip_suffix(mut self, suffix: &Path) -> Option<Self> {
		if !self.has_suffix(suffix) {
			return None;
		}

		self.parts.drain(self.parts.len() - suffix.parts.len()..);
		Some(self)
	}

	pub fn part(&self, index: usize) -> Option<&str> {
		self.parts.get(index).map(|s| s.as_str())
	}

	pub fn parts(&self) -> &[Arc<String>] {
		&self.parts
	}

	pub fn len(&self) -> usize {
		self.parts.len()
	}
}
