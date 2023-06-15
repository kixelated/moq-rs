mod client;
mod role;
mod server;
mod version;

pub use client::*;
pub use role::*;
pub use server::*;
pub use version::*;

// NOTE: These are forked from moq-transport-00.
//   1. messages lack a sized length
//   2. parameters are not optional and written in order (role + path)
//   3. role indicates local support only, not remote support
//   4. server setup is id=2 to disambiguate
