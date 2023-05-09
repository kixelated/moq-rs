use std::collections::VecDeque;

use anyhow;
use quiche;

#[derive(Default)]
pub struct Streams {
    ordered: Vec<Stream>,
}

struct Stream {
    id: u64,
    order: u64,

    buffer: VecDeque<u8>,
    fin: bool,
}

impl Streams {
    // Write the data to the given stream, buffering it if needed.
    pub fn send(
        &mut self,
        conn: &mut quiche::Connection,
        id: u64,
        buf: &[u8],
        fin: bool,
    ) -> anyhow::Result<()> {
        if buf.is_empty() && !fin {
            return Ok(())
        }

        // Get the index of the stream, or add it to the list of streams.
        let pos = self.ordered.iter().position(|s| s.id == id).unwrap_or_else(|| {
            // Create a new stream
            let stream = Stream{
                id,
                buffer: VecDeque::new(),
                fin: false,
                order: 0, // Default to highest priority until send_order is called.
            };

            self.insert(conn, stream)
        });

        let stream = &mut self.ordered[pos];

        // Check if we've already closed the stream, just in case.
        if stream.fin && !buf.is_empty() {
            anyhow::bail!("stream is already finished");
        }

        // If there's no data buffered, try to write it immediately.
        let size = if stream.buffer.is_empty() {
            conn.stream_send(id, buf, fin)?
        } else {
            0
        };

        if size < buf.len() {
            // Short write, save the rest for later.
            stream.buffer.extend(&buf[size..]);
        }

        stream.fin |= fin;

        Ok(())
    }

    // Flush any pending stream data.
    pub fn poll(&mut self, conn: &mut quiche::Connection) -> anyhow::Result<()> {
        // Loop over stream in order order.
        'outer: for stream in self.ordered.iter_mut() {
            // Keep reading from the buffer until it's empty.
            while !stream.buffer.is_empty() {
                // VecDeque is a ring buffer, so we can't write the whole thing at once.
                let parts = stream.buffer.as_slices();

                let size = conn.stream_send(stream.id, parts.0, false)?;
                if size == 0 {
                    // No more space available for this stream.
                    continue 'outer;
                }

                // Remove the bytes that were written.
                stream.buffer.drain(..size);
            }

            if stream.fin {
                // Write the stream done signal.
                conn.stream_send(stream.id, &[], true)?;
            }
        }

        // Remove streams that are done.
        // No need to reprioritize, since the streams are still in order order.
        self.ordered.retain(|stream| !stream.buffer.is_empty() || !stream.fin);

        Ok(())
    }

    // Set the send order of the stream.
    pub fn send_order(&mut self, conn: &mut quiche::Connection, id: u64, order: u64) {
        let mut stream = match self.ordered.iter().position(|s| s.id == id) {
            // Remove the stream from the existing list.
            Some(pos) => self.ordered.remove(pos),

            // This is a new stream, insert it into the list.
            None => Stream{
                id,
                buffer: VecDeque::new(),
                fin: false,
                order,
            },
        };

        stream.order = order;

        self.insert(conn, stream);
    }

    fn insert(&mut self, conn: &mut quiche::Connection, stream: Stream) -> usize {
        // Look for the position to insert the stream.
        let pos = match self.ordered.binary_search_by_key(&stream.order, |s| s.order) {
            Ok(pos) | Err(pos) => pos,
        };

        self.ordered.insert(pos, stream);

        // Reprioritize all later streams.
        // TODO we can avoid this if stream_priorty takes a u64
        for (i, stream) in self.ordered[pos..].iter().enumerate() {
            _ = conn.stream_priority(stream.id, (pos+i) as u8, true);
        }

        pos
    }
}
