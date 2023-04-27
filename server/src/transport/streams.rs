use std::collections::hash_map as hmap;
use std::collections::VecDeque;

use quiche;
use anyhow;

#[derive(Default)]
pub struct Streams {
    lookup: hmap::HashMap<u64, State>,
}

#[derive(Default)]
struct State {
    buffer: VecDeque<u8>,
    fin: bool,
}

impl Streams {
    pub fn send(&mut self, conn: &mut quiche::Connection, id: u64, buf: &[u8], fin: bool) -> anyhow::Result<()> {
        match self.lookup.entry(id) {
            hmap::Entry::Occupied(mut entry) => {
                // Add to the existing buffer.
                let state = entry.get_mut();
                state.buffer.extend(buf);
                state.fin |= fin;
            },
            hmap::Entry::Vacant(entry) => {
                let size = conn.stream_send(id, buf, fin)?;

                if size < buf.len() {
                    // Short write, save the rest for later.
                    let mut buffer = VecDeque::with_capacity(buf.len());
                    buffer.extend(&buf[size..]);

                    entry.insert(State{buffer, fin});
                }
            },
        };

        Ok(())
    }

    pub fn poll(&mut self, conn: &mut quiche::Connection) -> anyhow::Result<()> {
        'outer: for id in conn.writable() {
            // Check if there's any buffered data for this stream.
            let mut entry = match self.lookup.entry(id) {
                hmap::Entry::Occupied(entry) => entry,
                hmap::Entry::Vacant(_) => continue,
            };

            let state = entry.get_mut();

            // Keep reading from the buffer until it's empty.
            while state.buffer.len() > 0 {
                // VecDeque is a ring buffer, so we can't write the whole thing at once.
                let parts = state.buffer.as_slices();

                let size = conn.stream_send(id, parts.0, false)?;
                if size == 0 {
                    // No more space available for this stream.
                    continue 'outer
                }

                // Remove the bytes that were written.
                state.buffer.drain(..size);
            }

            if state.fin {
                // Write the stream done signal.
                conn.stream_send(id, &[], true)?;
            }

            // We can remove the value from the lookup once we've flushed everything.
            entry.remove();
        }

        Ok(())
    }
}