//! This module contains the structs and functions for the MoQ catalog format
mod consumer;
/// The catalog format is a JSON file that describes the tracks available in a broadcast.
///
/// The current version of the catalog format is draft-01.
/// https://www.ietf.org/archive/id/draft-ietf-moq-catalogformat-01.html
mod error;
mod producer;
mod root;
mod track;

pub use consumer::*;
pub use error::*;
pub use producer::*;
pub use root::*;
pub use track::*;
