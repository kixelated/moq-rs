//! This module contains the structs and functions for the MoQ catalog format
/// The catalog format is a JSON file that describes the tracks available in a broadcast.
mod audio;
mod broadcast;
mod dimensions;
mod error;
mod video;

pub use audio::*;
pub use broadcast::*;
pub use dimensions::*;
pub use error::*;
pub use video::*;
