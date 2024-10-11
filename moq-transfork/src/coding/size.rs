use bytes::{buf::UninitSlice, Buf, BufMut};

// A BufMut implementation that only counts the size of the buffer
#[derive(Default)]
pub struct Sizer {
	pub size: usize,
	buf: [u8; 16],
}

unsafe impl BufMut for Sizer {
	unsafe fn advance_mut(&mut self, cnt: usize) {
		self.size += cnt;
	}

	fn chunk_mut(&mut self) -> &mut UninitSlice {
		UninitSlice::new(&mut self.buf)
	}

	fn remaining_mut(&self) -> usize {
		usize::MAX
	}

	fn has_remaining_mut(&self) -> bool {
		true
	}

	fn put<T: Buf>(&mut self, mut src: T) {
		self.size += src.remaining();
		src.advance(src.remaining());
	}

	fn put_bytes(&mut self, _val: u8, cnt: usize) {
		self.size += cnt;
	}

	fn put_f32(&mut self, _val: f32) {
		self.size += 4;
	}

	fn put_f32_le(&mut self, _: f32) {
		self.size += 4
	}

	fn put_f32_ne(&mut self, _: f32) {
		self.size += 4
	}

	fn put_f64(&mut self, _: f64) {
		self.size += 8
	}

	fn put_f64_le(&mut self, _: f64) {
		self.size += 8
	}

	fn put_f64_ne(&mut self, _: f64) {
		self.size += 8
	}

	fn put_i128(&mut self, _: i128) {
		self.size += 16
	}

	fn put_i128_le(&mut self, _: i128) {
		self.size += 16
	}

	fn put_i128_ne(&mut self, _: i128) {
		self.size += 16
	}

	fn put_i16(&mut self, _: i16) {
		self.size += 2
	}

	fn put_i16_le(&mut self, _: i16) {
		self.size += 2
	}

	fn put_i16_ne(&mut self, _: i16) {
		self.size += 2
	}

	fn put_i32(&mut self, _: i32) {
		self.size += 4
	}

	fn put_i32_le(&mut self, _: i32) {
		self.size += 4
	}

	fn put_i32_ne(&mut self, _: i32) {
		self.size += 4
	}

	fn put_i64(&mut self, _: i64) {
		self.size += 8
	}

	fn put_i64_le(&mut self, _: i64) {
		self.size += 8
	}

	fn put_i64_ne(&mut self, _: i64) {
		self.size += 8
	}

	fn put_i8(&mut self, _: i8) {
		self.size += 1
	}

	fn put_int(&mut self, _: i64, nbytes: usize) {
		self.size += nbytes
	}

	fn put_int_le(&mut self, _: i64, nbytes: usize) {
		self.size += nbytes
	}

	fn put_int_ne(&mut self, _: i64, nbytes: usize) {
		self.size += nbytes
	}

	fn put_slice(&mut self, src: &[u8]) {
		self.size += src.len();
	}

	fn put_u128(&mut self, _: u128) {
		self.size += 16
	}

	fn put_u128_le(&mut self, _: u128) {
		self.size += 16
	}

	fn put_u128_ne(&mut self, _: u128) {
		self.size += 16
	}

	fn put_u16(&mut self, _: u16) {
		self.size += 2
	}

	fn put_u16_le(&mut self, _: u16) {
		self.size += 2
	}

	fn put_u16_ne(&mut self, _: u16) {
		self.size += 2
	}

	fn put_u32(&mut self, _: u32) {
		self.size += 4
	}

	fn put_u32_le(&mut self, _: u32) {
		self.size += 4
	}

	fn put_u32_ne(&mut self, _: u32) {
		self.size += 4
	}

	fn put_u64(&mut self, _: u64) {
		self.size += 8
	}

	fn put_u64_le(&mut self, _: u64) {
		self.size += 8
	}

	fn put_u64_ne(&mut self, _: u64) {
		self.size += 8
	}

	fn put_u8(&mut self, _: u8) {
		self.size += 1
	}

	fn put_uint(&mut self, _: u64, nbytes: usize) {
		self.size += nbytes
	}

	fn put_uint_le(&mut self, _: u64, nbytes: usize) {
		self.size += nbytes
	}

	fn put_uint_ne(&mut self, _: u64, nbytes: usize) {
		self.size += nbytes
	}

	// TODO
	// fn writer(self) -> bytes::buf::Writer<Self> {
	// fn chain_mut<U: BufMut>(self, next: U) -> bytes::buf::Chain<Self, U>
	// fn limit(self, limit: usize) -> bytes::buf::Limit<Self>
}
