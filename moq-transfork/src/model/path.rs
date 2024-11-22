use std::{fmt, sync::Arc};

#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Path {
	parts: Vec<Arc<String>>,
}

impl Path {
	pub const fn new() -> Self {
		Self { parts: Vec::new() }
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
}

impl std::ops::Deref for Path {
	type Target = [Arc<String>];

	fn deref(&self) -> &Self::Target {
		&self.parts
	}
}

impl fmt::Debug for Path {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "[")?;
		for (i, part) in self.parts.iter().enumerate() {
			if i > 0 {
				write!(f, ", ")?;
			}
			write!(f, "{:?}", part)?;
		}
		write!(f, "]")
	}
}

impl<S: ToString> FromIterator<S> for Path {
	fn from_iter<T: IntoIterator<Item = S>>(iter: T) -> Self {
		Self {
			parts: iter.into_iter().map(|t| t.to_string()).map(Arc::new).collect(),
		}
	}
}
