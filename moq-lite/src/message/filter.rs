use std::fmt;

use crate::coding::{Decode, DecodeError, Encode};

#[derive(Debug, Clone)]
pub enum Filter {
	// Allow all paths.
	Any,

	// Match an exact string.
	Exact(String),

	// Match a string prefix.
	Prefix(String),

	// Match a string suffix.
	Suffix(String),

	// Match a string with a wildcard in the middle.
	// The capture may be empty.
	Wildcard { prefix: String, suffix: String },
}

impl Filter {
	pub fn new(pattern: &str) -> Self {
		if pattern.is_empty() {
			return Self::Any;
		}

		if let Some((prefix, suffix)) = pattern.split_once("*") {
			return match (prefix.is_empty(), suffix.is_empty()) {
				(true, true) => Self::Any,
				(true, false) => Self::Suffix(suffix.to_string()),
				(false, true) => Self::Prefix(prefix.to_string()),
				(false, false) => Self::Wildcard {
					prefix: prefix.to_string(),
					suffix: suffix.to_string(),
				},
			};
		}

		Self::Exact(pattern.to_string())
	}

	/// Check if the input matches the filter.
	///
	/// Returns a [FilterMatch] that contains both the captured wildcard and the full match.
	pub fn matches<'a>(&self, input: &'a str) -> Option<FilterMatch<'a>> {
		match self {
			Self::Any => Some(FilterMatch {
				full: input,
				capture: (0, input.len()),
			}),
			Self::Exact(pattern) if input == pattern => Some(FilterMatch {
				full: input,
				capture: (0, 0),
			}),
			Self::Prefix(prefix) if input.starts_with(prefix) => Some(FilterMatch {
				full: input,
				capture: (prefix.len(), input.len()),
			}),
			Self::Suffix(suffix) if input.ends_with(suffix) => Some(FilterMatch {
				full: input,
				capture: (0, input.len() - suffix.len()),
			}),
			Self::Wildcard { prefix, suffix }
				if input.len() >= prefix.len() + suffix.len()
					&& input.starts_with(prefix)
					&& input.ends_with(suffix) =>
			{
				Some(FilterMatch {
					full: input,
					capture: (prefix.len(), input.len() - suffix.len()),
				})
			}
			_ => None,
		}
	}

	// Given a capture, reconstructs the full path.
	pub fn reconstruct(&self, capture: &str) -> String {
		match self {
			Self::Any => capture.to_string(),
			Self::Exact(pattern) => pattern.to_string(),
			Self::Prefix(prefix) => format!("{}{}", prefix, capture),
			Self::Suffix(suffix) => format!("{}{}", capture, suffix),
			Self::Wildcard { prefix, suffix } => format!("{}{}{}", prefix, capture, suffix),
		}
	}
}

impl<T: AsRef<str>> From<T> for Filter {
	fn from(pattern: T) -> Self {
		Self::new(pattern.as_ref())
	}
}

impl Encode for Filter {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		match self {
			Self::Any => {
				0u64.encode(w);
			}
			Self::Exact(pattern) => {
				pattern.encode(w);
			}
			Self::Prefix(prefix) => {
				(prefix.len() + 1).encode(w);
				w.put(prefix.as_bytes());
				w.put(&b"*"[..]);
			}
			Self::Suffix(suffix) => {
				(suffix.len() + 1).encode(w);
				w.put(&b"*"[..]);
				w.put(suffix.as_bytes());
			}
			Self::Wildcard { prefix, suffix } => {
				(prefix.len() + suffix.len() + 1).encode(w);
				w.put(prefix.as_bytes());
				w.put(&b"*"[..]);
				w.put(suffix.as_bytes());
			}
		}
	}
}

impl Decode for Filter {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let pattern = String::decode(r)?;
		Ok(Self::new(&pattern))
	}
}

#[cfg(test)]
impl Filter {
	fn assert(&self, input: &str, expected: Option<&str>) {
		let fm = self.matches(input).map(|r| r.capture());
		assert_eq!(fm, expected);
	}
}

#[derive(PartialEq, Eq)]
pub struct FilterMatch<'a> {
	full: &'a str,
	// An index into the string.
	capture: (usize, usize),
}

impl<'a> FilterMatch<'a> {
	pub fn full(&self) -> &'a str {
		self.full
	}

	pub fn capture(&self) -> &'a str {
		&self.full[self.capture.0..self.capture.1]
	}

	/// Returns the (start..end) index of the capture
	pub fn capture_index(&self) -> (usize, usize) {
		self.capture
	}
}

impl fmt::Debug for FilterMatch<'_> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("FilterMatch")
			.field("full", &self.full())
			.field("capture", &self.capture())
			.finish()
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn prefix() {
		let filter = Filter::new("*/bar/baz");
		filter.assert("foo/bar/baz", Some("foo"));
		filter.assert("foo/bar/", None);
		filter.assert("foo/bar/baz/qux", None);
		filter.assert("zoo/bar/baz", Some("zoo"));
	}

	#[test]
	fn middle() {
		let filter = Filter::new("foo/*/baz");
		filter.assert("foo/bar/baz", Some("bar"));
		filter.assert("foo/bar/", None);
		filter.assert("foo/bar/baz/qux", None);
		filter.assert("zoo/bar/baz", None);
	}

	#[test]
	fn suffix() {
		let filter = Filter::new("foo/bar/*");
		filter.assert("foo/bar/baz", Some("baz"));
		filter.assert("foo/bar/", Some(""));
		filter.assert("foo/bar/baz/qux", Some("baz/qux"));
		filter.assert("zoo/bar/baz", None);
	}

	#[test]
	fn literal() {
		let filter = Filter::new("foo/bar/baz");
		filter.assert("foo/bar/baz", Some(""));
		filter.assert("foo/bar/", None);
		filter.assert("foo/bar/baz/qux", None);
		filter.assert("zoo/bar/baz", None);
	}
}
