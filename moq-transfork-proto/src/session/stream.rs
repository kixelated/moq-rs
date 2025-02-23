use bytes::{Buf, BufMut};
use derive_more::{From, Into};

use super::Error;

#[enum_dispatch::enum_dispatch]
pub(crate) trait Stream {
	fn encode<B: BufMut>(&mut self, buf: &mut B);
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error>;
	fn closed(&mut self) -> Option<StreamCode>;
	fn close(&mut self, code: StreamCode);
}

#[derive(Debug, From, Into, PartialEq, Eq, Hash, Copy, Clone)]
pub struct StreamCode(pub u32);
