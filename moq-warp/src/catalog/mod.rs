//! This module contains the structs and functions for the MoQ catalog format
/// The catalog format is a JSON file that describes the tracks available in a broadcast.
///
/// The current version of the catalog format is draft-01.
/// https://www.ietf.org/archive/id/draft-ietf-moq-catalogformat-01.html
mod error;
mod reader;
mod root;
mod track;
mod writer;

pub use error::*;
pub use reader::*;
pub use root::*;
pub use track::*;
pub use writer::*;
